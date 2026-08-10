use std::{collections::BTreeSet, fs, path::Path};

use xtask::ToolResult;

use crate::{
    document::{is_external_link, markdown_targets},
    repository::{
        ensure_regular_directory, ensure_regular_file, include_targets, repository_files,
        same_file, workspace_members,
    },
};

pub(crate) fn run(root: &Path) -> ToolResult<()> {
    let agents = root.join("AGENTS.md");
    ensure_regular_file(&agents, "根 AGENTS.md")?;
    let agents_text = fs::read_to_string(&agents)?;
    super::agents::check_agents_document_contract(root, &agents_text)?;
    check_documentation_entrypoints(root, &agents_text)?;
    check_markdown_links(root)?;
    check_include_str_targets(root)?;
    check_crate_readme_includes(root, &agents_text)?;
    check_adr_index(root)?;
    println!(
        "ok: 文档入口、Context glossary、文档链接、include_str!、crate README、ADR index 和 workspace crate map 已通过"
    );
    Ok(())
}

fn check_documentation_entrypoints(root: &Path, agents_text: &str) -> ToolResult<()> {
    let context = root.join("CONTEXT.md");
    let documentation = root.join("docs/documentation.md");
    ensure_regular_file(&context, "根 CONTEXT.md")?;
    ensure_regular_file(&documentation, "文档治理指南")?;

    for (label, marker) in [
        ("CONTEXT.md", "(CONTEXT.md)"),
        ("docs/documentation.md", "(docs/documentation.md)"),
    ] {
        if !agents_text.contains(marker) {
            return Err(
                std::io::Error::other(format!("根 AGENTS.md 缺少文档入口链接: {label}")).into(),
            );
        }
    }

    check_context_document(&context)
}

#[derive(Debug)]
struct ContextTerm {
    display: String,
    line: usize,
    has_definition: bool,
    has_avoid: bool,
}

fn check_context_document(path: &Path) -> ToolResult<()> {
    let text = fs::read_to_string(path)?;
    let mut saw_title = false;
    let mut language_sections = 0usize;
    let mut in_language = false;
    let mut current: Option<ContextTerm> = None;
    let mut canonical_aliases = BTreeSet::new();
    let mut avoid_terms = BTreeSet::new();

    for (index, raw_line) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(level) = markdown_heading_level(line) {
            match level {
                1 => {
                    if saw_title {
                        return context_error(path, line_number, "CONTEXT.md 只能有一个根标题");
                    }
                    if in_language {
                        return context_error(path, line_number, "Language 后不得出现新的根标题");
                    }
                    saw_title = true;
                }
                2 => {
                    if line == "## Language" {
                        if !saw_title {
                            return context_error(
                                path,
                                line_number,
                                "## Language 必须位于根标题之后",
                            );
                        }
                        if language_sections > 0 {
                            return context_error(
                                path,
                                line_number,
                                "CONTEXT.md 只能有一个 ## Language",
                            );
                        }
                        finish_context_term(&mut current, path)?;
                        language_sections += 1;
                        in_language = true;
                    } else if in_language {
                        return context_error(path, line_number, "Language 中不允许其他二级标题");
                    }
                }
                _ => {
                    if !in_language {
                        return context_error(path, line_number, "Language 之前不允许分组标题");
                    }
                    finish_context_term(&mut current, path)?;
                }
            }
            continue;
        }

        if !in_language {
            continue;
        }

        if line.starts_with("**") {
            let (display, aliases) = parse_context_term_heading(line).map_err(|message| {
                std::io::Error::other(format!("{}:{}: {message}", path.display(), line_number))
            })?;
            finish_context_term(&mut current, path)?;
            for alias in &aliases {
                if !canonical_aliases.insert(alias.clone()) {
                    return context_error(
                        path,
                        line_number,
                        format!("canonical term 重复: {display}"),
                    );
                }
            }
            current = Some(ContextTerm {
                display,
                line: line_number,
                has_definition: false,
                has_avoid: false,
            });
            continue;
        }

        if let Some(rest) = line.strip_prefix("_Avoid_:") {
            let Some(term) = current.as_mut() else {
                return context_error(path, line_number, "_Avoid_: 必须位于词条之后");
            };
            if term.has_avoid {
                return context_error(
                    path,
                    line_number,
                    format!("词条重复声明 _Avoid_: {}", term.display),
                );
            }
            let values = rest
                .split([',', '，', '、'])
                .map(str::trim)
                .collect::<Vec<_>>();
            if values.is_empty() || values.iter().any(|value| value.is_empty()) {
                return context_error(path, line_number, "_Avoid_: 必须包含非空词语");
            }
            for value in values {
                let normalized = normalize_context_name(value);
                if !avoid_terms.insert(normalized) {
                    return context_error(path, line_number, format!("avoid term 重复: {value}"));
                }
            }
            term.has_avoid = true;
            continue;
        }

        if let Some(term) = current.as_mut() {
            term.has_definition = true;
        } else {
            return context_error(path, line_number, "Language 中出现了未归属词条的内容");
        }
    }

    if !saw_title {
        return context_error(path, 1, "缺少根标题");
    }
    if language_sections != 1 {
        return context_error(path, 1, "必须有且只有一个 ## Language");
    }
    finish_context_term(&mut current, path)?;
    if canonical_aliases.is_empty() {
        return context_error(path, 1, "Language 至少需要一个词条");
    }
    if let Some(conflict) = avoid_terms.intersection(&canonical_aliases).next().cloned() {
        return context_error(
            path,
            1,
            format!("avoid term 不得与 canonical term 冲突: {conflict}"),
        );
    }
    Ok(())
}

fn markdown_heading_level(line: &str) -> Option<usize> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    (level > 0 && line.chars().nth(level) == Some(' ')).then_some(level)
}

fn parse_context_term_heading(line: &str) -> Result<(String, Vec<String>), &'static str> {
    if !line.ends_with("**:") || line.len() < 6 {
        return Err("词条必须使用 **中文名（literal）**: 格式");
    }
    let body = &line[2..line.len() - 3];
    let Some((display_name, literal_tail)) = body.split_once('（') else {
        return Err("词条必须包含中文名和全角括号中的 literal");
    };
    let Some(literal) = literal_tail.strip_suffix('）') else {
        return Err("词条 literal 必须以全角右括号结束");
    };
    let display_name = display_name.trim();
    let literal = literal.trim();
    if display_name.is_empty() || literal.is_empty() || body.contains("**") {
        return Err("词条中文名和 literal 不能为空");
    }
    let display = body.trim().to_owned();
    let aliases = [display.as_str(), display_name, literal]
        .into_iter()
        .map(normalize_context_name)
        .collect::<Vec<_>>();
    if aliases.iter().any(String::is_empty) {
        return Err("词条 canonical name 不能为空");
    }
    Ok((display, aliases))
}

fn normalize_context_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn finish_context_term(current: &mut Option<ContextTerm>, path: &Path) -> ToolResult<()> {
    if let Some(term) = current.take()
        && !term.has_definition
    {
        return context_error(
            path,
            term.line,
            format!("词条定义不能为空: {}", term.display),
        );
    }
    Ok(())
}

fn context_error<T>(path: &Path, line: usize, message: impl Into<String>) -> ToolResult<T> {
    Err(std::io::Error::other(format!("{}:{}: {}", path.display(), line, message.into())).into())
}

fn check_markdown_links(root: &Path) -> ToolResult<()> {
    for path in repository_files(root, "md")? {
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

fn check_include_str_targets(root: &Path) -> ToolResult<()> {
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

fn check_crate_readme_includes(root: &Path, agents_text: &str) -> ToolResult<()> {
    super::agents::check_workspace_map(root, agents_text)?;
    let members = workspace_members(root)?;
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
            .collect::<ToolResult<Vec<_>>>()?
            .into_iter()
            .any(|included| included);
        if !included {
            return Err(std::io::Error::other(format!(
                "workspace crate 缺少 README include_str!: {member}"
            ))
            .into());
        }
    }
    Ok(())
}

fn check_adr_index(root: &Path) -> ToolResult<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, time::SystemTime};

    fn temp_root(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("xtask-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("temporary root should be creatable");
        path
    }

    fn valid_context() -> &'static str {
        "# Context\n\nA glossary.\n\n## Language\n\n### Core\n\n**看板（Board）**:\n任务隔离边界。\n\n**任务（Task）**:\n看板中的规范工作项。\n_Avoid_: card, issue\n"
    }

    #[test]
    fn context_lint_accepts_language_groups_and_avoid_terms() {
        let root = temp_root("context-valid");
        let path = root.join("CONTEXT.md");
        fs::write(&path, valid_context()).expect("context should be writable");

        assert!(check_context_document(&path).is_ok());

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn context_lint_rejects_missing_language_section() {
        let root = temp_root("context-language");
        let path = root.join("CONTEXT.md");
        fs::write(&path, "# Context\n\nNo language section.\n")
            .expect("context should be writable");

        assert!(check_context_document(&path).is_err());

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn context_lint_rejects_empty_definition() {
        let root = temp_root("context-definition");
        let path = root.join("CONTEXT.md");
        fs::write(
            &path,
            "# Context\n\n## Language\n\n### Core\n\n**看板（Board）**:\n\n**任务（Task）**:\n规范工作项。\n",
        )
        .expect("context should be writable");

        assert!(check_context_document(&path).is_err());

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn context_lint_rejects_duplicate_canonical_term() {
        let root = temp_root("context-canonical");
        let path = root.join("CONTEXT.md");
        fs::write(
            &path,
            "# Context\n\n## Language\n\n**看板（Board）**:\n隔离边界。\n\n**工作区（Board）**:\n另一种写法。\n",
        )
        .expect("context should be writable");

        assert!(check_context_document(&path).is_err());

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn context_lint_rejects_duplicate_and_conflicting_avoid_terms() {
        let root = temp_root("context-avoid");
        let duplicate = root.join("duplicate.md");
        fs::write(
            &duplicate,
            "# Context\n\n## Language\n\n**看板（Board）**:\n隔离边界。\n_Avoid_: card, card\n",
        )
        .expect("context should be writable");
        assert!(check_context_document(&duplicate).is_err());

        let conflict = root.join("conflict.md");
        fs::write(
            &conflict,
            "# Context\n\n## Language\n\n**看板（Board）**:\n隔离边界。\n_Avoid_: Board\n",
        )
        .expect("context should be writable");
        assert!(check_context_document(&conflict).is_err());

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn documentation_entrypoints_require_context_and_guide_links() {
        let root = temp_root("documentation-entrypoints");
        fs::create_dir_all(root.join("docs")).expect("docs directory should be creatable");
        fs::write(root.join("CONTEXT.md"), valid_context()).expect("context should be writable");
        fs::write(root.join("docs/documentation.md"), "# Documentation\n")
            .expect("documentation guide should be writable");

        assert!(
            check_documentation_entrypoints(
                &root,
                "[Context](CONTEXT.md)\n[Guide](docs/documentation.md)"
            )
            .is_ok()
        );
        assert!(check_documentation_entrypoints(&root, "[Context](CONTEXT.md)").is_err());

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    #[test]
    fn markdown_links_check_archived_directories() {
        let root = temp_root("markdown-archives");
        fs::create_dir_all(root.join("docs/release"))
            .expect("release directory should be creatable");
        fs::create_dir_all(root.join("docs/migration"))
            .expect("migration directory should be creatable");
        fs::write(
            root.join("docs/release/README.md"),
            "[broken](missing-release.md)\n",
        )
        .expect("release markdown should be writable");
        fs::write(
            root.join("docs/migration/README.md"),
            "[broken](missing-migration.md)\n",
        )
        .expect("migration markdown should be writable");

        assert!(check_markdown_links(&root).is_err());

        fs::remove_file(root.join("docs/release/README.md"))
            .expect("release markdown should be removable");
        assert!(check_markdown_links(&root).is_err());

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
}
