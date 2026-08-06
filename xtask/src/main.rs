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
        ("docs", Some("check")) => run_docs_check(&root),
        ("deps", Some("check")) => run_deps_check(&root),
        ("agents", Some("check")) => run_agents_check(&root),
        ("schema", None) => invalid("schema 缺少子命令"),
        ("docs", Some(command)) => invalid(format!("docs 不支持子命令: {command}")),
        ("docs", None) => invalid("docs 缺少子命令"),
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
        serde_json::to_string_pretty(&adopted).expect("operation inventory 必须可序列化")
    );
}

fn run_deps_check(root: &Path) -> xtask::ToolResult<()> {
    run_checked(root, "python3", ["-B", "scripts/test_dependency_owners.py"])?;
    run_checked(
        root,
        "python3",
        ["-B", "scripts/schema_dependency_policy.py"],
    )?;
    run_checked(
        root,
        "python3",
        ["-B", "scripts/check-dependency-owners.py"],
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
    let text = fs::read_to_string(&agents)?;
    check_agents_document_contract(root, &text)?;
    check_skill_packages(root)?;
    check_active_maps(root)?;
    println!("ok: AGENTS.md、技能包结构和 active recipe/package map 已通过");
    Ok(())
}

fn run_docs_check(root: &Path) -> xtask::ToolResult<()> {
    let agents = root.join("AGENTS.md");
    ensure_regular_file(&agents, "根 AGENTS.md")?;
    let agents_text = fs::read_to_string(&agents)?;
    check_agents_document_contract(root, &agents_text)?;
    check_markdown_links(root)?;
    check_include_str_targets(root)?;
    check_crate_readme_includes(root, &agents_text)?;
    check_adr_index(root)?;
    println!("ok: 文档链接、include_str!、crate README、ADR index 和 workspace crate map 已通过");
    Ok(())
}

const REQUIRED_AGENT_SECTIONS: &[&str] = &[
    "## 1. 产品边界",
    "## 2. 稳定不变量",
    "## 3. 工作区地图",
    "## 4. 任务边界与停止",
    "## 5. 技能路由",
    "## 6. 文档地图",
    "## 7. 验证边界",
    "## 8. 语言与 Git 边界",
    "## 9. 维护",
];

const REQUIRED_SKILL_ROUTES: &[&str] = &["$style", "$prose", "$docs", "$check", "$commit"];

fn check_agents_document_contract(_root: &Path, text: &str) -> xtask::ToolResult<()> {
    for heading in REQUIRED_AGENT_SECTIONS {
        if !text.lines().any(|line| line.trim() == *heading) {
            return Err(
                std::io::Error::other(format!("根 AGENTS.md 缺少必要 section: {heading}")).into(),
            );
        }
    }
    let skill_section = section_body(text, "## 5. 技能路由")?;
    for route in REQUIRED_SKILL_ROUTES {
        if !section_contains_bullet(skill_section, route) {
            return Err(
                std::io::Error::other(format!("根 AGENTS.md 缺少技能路由: {route}")).into(),
            );
        }
    }
    Ok(())
}

fn section_body<'a>(text: &'a str, heading: &str) -> xtask::ToolResult<&'a str> {
    let (_, body) = text.split_once(heading).ok_or_else(|| {
        std::io::Error::other(format!("根 AGENTS.md 缺少必要 section: {heading}"))
    })?;
    Ok(body.split_once("\n## ").map_or(body, |(body, _)| body))
}

fn section_contains_bullet(section: &str, needle: &str) -> bool {
    section
        .lines()
        .any(|line| line.trim_start().starts_with("- ") && line.contains(needle))
}

fn check_markdown_links(root: &Path) -> xtask::ToolResult<()> {
    for path in repository_files(root, "md")? {
        if is_archived_markdown(root, &path) {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        for target in markdown_targets(&text) {
            if target.is_empty() || target.starts_with('#') || is_external_link(&target) {
                continue;
            }
            let target_path = target.split('#').next().unwrap_or_default();
            if target_path.is_empty() {
                continue;
            }
            let candidate = path.parent().unwrap_or(root).join(target_path);
            if !candidate.exists() {
                return Err(std::io::Error::other(format!(
                    "Markdown 本地链接不存在: {} -> {}",
                    path.strip_prefix(root).unwrap_or(&path).display(),
                    target
                ))
                .into());
            }
        }
    }
    Ok(())
}

fn check_include_str_targets(root: &Path) -> xtask::ToolResult<()> {
    for path in repository_files(root, "rs")? {
        let text = fs::read_to_string(&path)?;
        for target in include_targets(root, &path, &text)? {
            // `concat!(env!("CARGO_MANIFEST_DIR"), "/base/", $literal)` 在宏定义处
            // 只能解析到基础目录；具体文件由每个宏展开和 rustdoc 编译继续校验。
            if target.is_dir() {
                continue;
            }
            ensure_regular_file(&target, "include_str! 目标")?;
        }
    }
    Ok(())
}

fn check_crate_readme_includes(root: &Path, agents_text: &str) -> xtask::ToolResult<()> {
    let members = workspace_members(root)?;
    let workspace_section = section_body(agents_text, "## 3. 工作区地图")?;
    for member in members {
        let member_root = root.join(&member);
        ensure_regular_directory(&member_root, "workspace crate")?;
        let readme = if member == "apps/desktop/src-tauri" {
            root.join("apps/desktop/README.md")
        } else {
            member_root.join("README.md")
        };
        ensure_regular_file(&readme, "workspace crate README")?;

        let included = repository_files(&member_root, "rs")?
            .into_iter()
            .map(|source| {
                let text = fs::read_to_string(&source)?;
                Ok(include_targets(root, &source, &text)?
                    .into_iter()
                    .any(|target| same_file(&target, &readme)))
            })
            .collect::<xtask::ToolResult<Vec<_>>>()?
            .into_iter()
            .any(|included| included);
        if !included {
            return Err(std::io::Error::other(format!(
                "workspace crate 缺少 README include_str!: {member}"
            ))
            .into());
        }

        let map_key = member
            .rsplit('/')
            .next()
            .filter(|name| *name != "src-tauri")
            .unwrap_or(member.as_str());
        if !section_contains_bullet(workspace_section, map_key)
            && !section_contains_bullet(workspace_section, &member)
        {
            return Err(std::io::Error::other(format!(
                "根 AGENTS.md 工作区地图缺少 workspace member: {member}"
            ))
            .into());
        }
    }
    Ok(())
}

fn check_adr_index(root: &Path) -> xtask::ToolResult<()> {
    let directory = root.join("docs/adr");
    let index = directory.join("README.md");
    ensure_regular_file(&index, "ADR index")?;
    let index_text = fs::read_to_string(&index)?;
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md")
            || path.file_name().and_then(|value| value.to_str()) == Some("README.md")
        {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !index_text.contains(file_name) {
            return Err(std::io::Error::other(format!("ADR index 未列出文件: {file_name}")).into());
        }
    }
    Ok(())
}

fn workspace_members(root: &Path) -> xtask::ToolResult<Vec<String>> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))?;
    let (_, remainder) = manifest
        .split_once("members = [")
        .ok_or_else(|| std::io::Error::other("Cargo.toml 缺少 workspace members"))?;
    let members_text = remainder
        .split_once(']')
        .map(|(value, _)| value)
        .ok_or_else(|| std::io::Error::other("workspace members 未闭合"))?;
    let mut members = Vec::new();
    for line in members_text.lines() {
        let line = line.trim();
        if let Some(value) = line
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix(','))
        {
            if let Some(value) = value.strip_suffix('"') {
                members.push(value.to_owned());
            }
        }
    }
    if members.is_empty() {
        return Err(std::io::Error::other("workspace members 为空").into());
    }
    Ok(members)
}

fn repository_files(root: &Path, extension: &str) -> xtask::ToolResult<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    collect_files(root, extension, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(
    path: &Path,
    extension: &str,
    files: &mut Vec<std::path::PathBuf>,
) -> xtask::ToolResult<()> {
    if path.is_symlink() {
        return Ok(());
    }
    if path.is_file() {
        if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            files.push(path.to_owned());
        }
        return Ok(());
    }
    if !path.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some(".git" | "target" | "node_modules" | ".pnpm")
        ) {
            continue;
        }
        collect_files(&entry.path(), extension, files)?;
    }
    Ok(())
}

fn is_archived_markdown(root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.starts_with("docs/release") || relative.starts_with("docs/migration")
}

fn markdown_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("](") {
        let after = &remaining[start + 2..];
        let Some(end) = after.find(')') else { break };
        let raw = after[..end].trim();
        let raw = raw
            .strip_prefix('<')
            .and_then(|value| value.strip_suffix('>'))
            .unwrap_or(raw);
        if let Some(target) = raw.split_whitespace().next() {
            targets.push(target.trim_matches('"').to_owned());
        }
        remaining = &after[end + 1..];
    }
    targets
}

fn is_external_link(target: &str) -> bool {
    target.starts_with("//")
        || target.starts_with('/')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with("file:")
}

fn include_targets(
    root: &Path,
    source: &Path,
    text: &str,
) -> xtask::ToolResult<Vec<std::path::PathBuf>> {
    let mut targets = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("include_str!(") {
        if remaining[..start].chars().last() == Some('"') {
            remaining = &remaining[start + "include_str!(".len()..];
            continue;
        }
        let after = remaining[start + "include_str!(".len()..].trim_start();
        if !after.starts_with('"') {
            if after.starts_with("concat!(") && after.contains("CARGO_MANIFEST_DIR") {
                if let Some(relative) = after
                    .split('"')
                    .filter(|value| value.starts_with("/"))
                    .next()
                {
                    if let Some(manifest_dir) = source
                        .ancestors()
                        .find(|candidate| candidate.join("Cargo.toml").is_file())
                    {
                        targets.push(manifest_dir.join(relative.trim_start_matches('/')));
                    }
                }
            }
            remaining = after;
            continue;
        }
        let after_quote = &after[1..];
        let Some(end) = after_quote.find('"') else {
            return Err(std::io::Error::other(format!(
                "include_str! 字符串未闭合: {}",
                source.strip_prefix(root).unwrap_or(source).display()
            ))
            .into());
        };
        let relative = &after_quote[..end];
        if relative.contains('\\') {
            return Err(
                std::io::Error::other(format!("include_str! 不支持转义路径: {relative}")).into(),
            );
        }
        let target = source.parent().unwrap_or(root).join(relative);
        targets.push(target);
        remaining = &after_quote[end + 1..];
    }
    Ok(targets)
}

fn same_file(left: &Path, right: &Path) -> bool {
    fs::canonicalize(left).ok() == fs::canonicalize(right).ok()
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
        "scripts/check-dependency-owners.py",
        "scripts/test_dependency_owners.py",
        "scripts/test-schema-cargo-tree.sh",
        "scripts/test_schema_recipe_witness.py",
    ];
    const STALE_REFERENCES: &[&str] = &["kanban-schema-tool", "kanban-sqlite", "kanban-local"];
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
        "用法：xtask <docs check|schema generate|check|audit|witnesses|deps check|agents check> [--root PATH] [--require-closed]"
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

    fn write_agents(root: &Path) {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../AGENTS.md");
        let text = fs::read_to_string(source).expect("canonical AGENTS should be readable");
        fs::write(root.join("AGENTS.md"), text).expect("AGENTS should be writable");
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
    fn repository_files_skips_dependency_and_build_trees() {
        let root = temp_root("repository-files");
        fs::create_dir_all(root.join("docs")).expect("docs directory should be creatable");
        fs::create_dir_all(root.join("node_modules/pkg"))
            .expect("node_modules directory should be creatable");
        fs::create_dir_all(root.join("target/doc")).expect("target directory should be creatable");
        fs::write(root.join("docs/guide.md"), "# Guide\n").expect("guide should be writable");
        fs::write(
            root.join("node_modules/pkg/README.md"),
            "[broken](../missing)\n",
        )
        .expect("dependency README should be writable");
        fs::write(
            root.join("target/doc/generated.md"),
            "[broken](../missing)\n",
        )
        .expect("generated README should be writable");

        let files = repository_files(&root, "md").expect("repository files should be readable");
        assert_eq!(files, vec![root.join("docs/guide.md")]);

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn include_targets_accept_existing_dynamic_base_directory() {
        let root = temp_root("dynamic-include");
        let crate_root = root.join("crates/example");
        fs::create_dir_all(crate_root.join("src")).expect("crate source should be creatable");
        fs::create_dir_all(root.join("schemas/fixtures"))
            .expect("fixture directory should be creatable");
        fs::write(
            crate_root.join("Cargo.toml"),
            "[package]\nname = \"example\"\n",
        )
        .expect("manifest should be writable");
        fs::write(
            crate_root.join("src/lib.rs"),
            r#"include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/",
                $path
            ));"#,
        )
        .expect("source should be writable");

        assert!(check_include_str_targets(&root).is_ok());
        fs::remove_dir_all(root.join("schemas/fixtures"))
            .expect("fixture directory should be removable");
        assert!(check_include_str_targets(&root).is_err());

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn agents_check_is_fail_closed_for_skill_layout_and_openai_contract() {
        let root = temp_root("agents");
        write_agents(&root);

        for skill in ["prose", "docs", "check", "commit", "style"] {
            write_skill(&root, skill);
        }
        assert!(run_agents_check(&root).is_ok());

        let agents_path = root.join("AGENTS.md");
        let canonical = fs::read_to_string(&agents_path).expect("AGENTS should be readable");
        fs::write(
            &agents_path,
            format!("{canonical}\n{}", "extra\n".repeat(200)),
        )
        .expect("long AGENTS should be writable");
        assert!(run_agents_check(&root).is_ok());

        fs::write(
            &agents_path,
            canonical.replace("## 5. 技能路由", "## 5. missing"),
        )
        .expect("missing section should be writable");
        assert!(run_agents_check(&root).is_err());

        fs::write(
            &agents_path,
            canonical.replace(
                "- `$style`：Rust、Cargo、模块组织、依赖边界、错误和测试位置。",
                "- style：Rust、Cargo、模块组织、依赖边界、错误和测试位置。",
            ),
        )
        .expect("missing skill route should be writable");
        assert!(run_agents_check(&root).is_err());

        fs::write(
            &agents_path,
            canonical.replace("kanban-service", "retired-service"),
        )
        .expect("missing workspace map entry should be writable");
        assert!(run_agents_check(&root).is_err());

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
