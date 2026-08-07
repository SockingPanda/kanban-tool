use std::{fs, path::Path};

use xtask::ToolResult;

use crate::{
    document::{section_body, section_contains_bullet},
    repository::{
        ensure_regular_directory, ensure_regular_file, required_directory, workspace_members,
    },
};

pub(crate) fn run(root: &Path) -> ToolResult<()> {
    let agents = root.join("AGENTS.md");
    ensure_regular_file(&agents, "根 AGENTS.md")?;
    let text = fs::read_to_string(&agents)?;
    check_agents_document_contract(root, &text)?;
    check_workspace_map(root, &text)?;
    check_skill_packages(root)?;
    check_active_maps(root)?;
    println!("ok: AGENTS.md、技能包结构和 active recipe/package map 已通过");
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

pub(crate) fn check_agents_document_contract(_root: &Path, text: &str) -> ToolResult<()> {
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

pub(crate) fn check_workspace_map(root: &Path, agents_text: &str) -> ToolResult<()> {
    let workspace_section = section_body(agents_text, "## 3. 工作区地图")?;
    for member in workspace_members(root)? {
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

fn check_skill_packages(root: &Path) -> ToolResult<()> {
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

fn check_skill_contract(skill: &str, text: &str) -> ToolResult<()> {
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
    let description = frontmatter
        .lines()
        .find_map(|line| line.strip_prefix("description:").map(str::trim))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            std::io::Error::other(format!("技能包 {skill} frontmatter 缺少非空 description:"))
        })?;
    if !description_expresses_trigger(description) {
        return Err(
            std::io::Error::other(format!("技能包 {skill} description 必须表达触发条件")).into(),
        );
    }
    Ok(())
}

fn description_expresses_trigger(description: &str) -> bool {
    const TRIGGER_TERMS: &[&str] = &[
        "when",
        "if ",
        "适用于",
        "用于",
        "当",
        "在",
        "只有",
        "需要",
        "明确",
        "为",
    ];
    const ACTION_TERMS: &[&str] = &[
        "Use", "run", "check", "build", "create", "write", "review", "maintain", "apply",
        "execute", "use", "使用", "运行", "验证", "改动", "选择", "执行", "维护", "判断", "编写",
        "重写", "描述", "新写", "修改", "起草", "创建", "检查", "保持", "提交",
    ];
    let description = description.trim();
    description.chars().count() >= 4
        && TRIGGER_TERMS.iter().any(|term| description.contains(term))
        && ACTION_TERMS.iter().any(|term| description.contains(term))
}

fn check_openai_contract(skill: &str, path: &Path) -> ToolResult<()> {
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

fn check_active_maps(root: &Path) -> ToolResult<()> {
    const ACTIVE_MAPS: &[&str] = &[
        "justfile",
        "xtask/src/affected.rs",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, path::PathBuf, time::SystemTime};

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("xtask-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("temporary root should be creatable");
        path
    }

    fn skill_text(name: &str, description: &str, body: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n")
    }

    fn write_skill(root: &Path, name: &str) {
        let skill = root.join(".agents/skills").join(name);
        fs::create_dir_all(skill.join("agents")).expect("skill directory should be creatable");
        fs::write(
            skill.join("SKILL.md"),
            skill_text(name, "用于在需要验证时运行此 skill", "# 自定义正文结构"),
        )
        .expect("skill contract should be writable");
        fs::write(
            skill.join("agents/openai.yaml"),
            "interface: conversation\ndisplay_name: Test\nshort_description: Test\ndefault_prompt: Test\n",
        )
        .expect("openai contract should be writable");
    }

    fn write_agents(root: &Path) {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let agents = fs::read_to_string(repository.join("AGENTS.md"))
            .expect("canonical AGENTS should be readable");
        fs::write(root.join("AGENTS.md"), agents).expect("AGENTS should be writable");
        let manifest = fs::read_to_string(repository.join("Cargo.toml"))
            .expect("workspace manifest should be readable");
        fs::write(root.join("Cargo.toml"), manifest)
            .expect("workspace manifest should be writable");
        for (index, member) in workspace_members(&repository)
            .unwrap()
            .into_iter()
            .enumerate()
        {
            let member_root = root.join(member);
            fs::create_dir_all(member_root.join("src"))
                .expect("workspace fixture directory should be creatable");
            fs::write(
                member_root.join("Cargo.toml"),
                format!(
                    "[package]\nname = \"xtask-fixture-{index}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"
                ),
            )
            .expect("workspace fixture manifest should be writable");
            fs::write(member_root.join("src/lib.rs"), "")
                .expect("workspace source should be writable");
        }
    }

    #[test]
    fn skill_contract_requires_frontmatter_and_trigger_description_but_not_headings() {
        assert!(
            check_skill_contract(
                "check",
                &skill_text("wrong", "用于在需要验证时运行此 skill", "任意正文")
            )
            .is_err()
        );
        assert!(
            check_skill_contract("check", &skill_text("check", "generic skill", "任意正文"))
                .is_err()
        );
        assert!(check_skill_contract("check", &skill_text("check", "", "任意正文")).is_err());
        assert!(check_skill_contract("check", &skill_text("check", "用于", "任意正文")).is_err());
        assert!(!description_expresses_trigger("当"));
        assert!(
            check_skill_contract(
                "check",
                &skill_text("check", "用于在需要验证时运行此 skill", "任意正文")
            )
            .is_ok()
        );
    }

    #[test]
    fn agents_check_is_fail_closed_for_skill_layout_and_openai_contract() {
        let root = temp_root("agents");
        write_agents(&root);

        for skill in ["prose", "docs", "check", "commit", "style"] {
            write_skill(&root, skill);
        }
        assert!(run(&root).is_ok());

        let agents_path = root.join("AGENTS.md");
        let canonical = fs::read_to_string(&agents_path).expect("AGENTS should be readable");
        fs::write(
            &agents_path,
            format!("{canonical}\n{}", "extra\n".repeat(200)),
        )
        .expect("long AGENTS should be writable");
        assert!(run(&root).is_ok());

        fs::write(
            &agents_path,
            canonical.replace("## 5. 技能路由", "## 5. missing"),
        )
        .expect("missing section should be writable");
        assert!(run(&root).is_err());

        fs::write(
            &agents_path,
            canonical.replace(
                "- `$style`：Rust、Cargo、模块组织、依赖边界、错误和测试位置。",
                "- style：Rust、Cargo、模块组织、依赖边界、错误和测试位置。",
            ),
        )
        .expect("missing skill route should be writable");
        assert!(run(&root).is_err());

        fs::write(
            &agents_path,
            canonical.replace("kanban-service", "retired-service"),
        )
        .expect("missing workspace map entry should be writable");
        assert!(run(&root).is_err());

        write_skill(&root, "extra");
        assert!(run(&root).is_err());
        fs::remove_dir_all(root.join(".agents/skills/extra"))
            .expect("extra skill should be removable");
        fs::remove_file(root.join(".agents/skills/check/agents/openai.yaml"))
            .expect("openai contract should be removable");
        assert!(run(&root).is_err());

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}
