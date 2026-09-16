//! 校验根技能路由与仓库内技能引用；不调用模型、不执行技能正文。
//!
//! 此处只识别仓库约定的路由写法与代码跨度中的 `$skill-name`。
//! 跳过 fenced code 与可辨认的 Shell 形式；单独的 `$lowercase` 按约定作为技能引用。

use std::{collections::BTreeSet, fs, io, path::Path};

const ROUTE_HEADING: &str = "## 5. 技能路由";
const GOVERNANCE_DOCUMENTS: &[&str] = &["docs/documentation.md", "docs/collaboration.md"];

/// 根路由、实际技能目录和技能正文引用必须闭合。
///
/// frontmatter 和 openai.yaml 仍由父模块原有检查负责。
/// 不从全局用户目录补齐缺失技能，避免让仓库依赖某台机器的配置。
pub(super) fn check(root: &Path, agents: &str) -> io::Result<()> {
    let routes = route_names(agents)?;
    let directory = root.join(".agents/skills");
    require_kind(&directory, true)?;
    let mut packages = BTreeSet::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        require_kind(&entry.path(), true)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| io::Error::other("技能目录名必须是 UTF-8"))?;
        if !valid_name(&name) {
            return Err(io::Error::other(format!("非法技能目录名: {name}")));
        }
        packages.insert(name);
    }
    compare_routes(&routes, &packages)?;
    validate_references("AGENTS.md", agents, &packages)?;
    for name in &packages {
        let file = directory.join(name).join("SKILL.md");
        require_kind(&file, false)?;
        let text = fs::read_to_string(&file)?;
        validate_references(&format!(".agents/skills/{name}/SKILL.md"), &text, &packages)?;
    }
    for relative in GOVERNANCE_DOCUMENTS {
        let path = root.join(relative);
        require_kind(&path, false)?;
        validate_references(relative, &fs::read_to_string(path)?, &packages)?;
    }
    Ok(())
}

fn validate_references(label: &str, text: &str, packages: &BTreeSet<String>) -> io::Result<()> {
    for reference in inline_references(text) {
        if !packages.contains(&reference) {
            return Err(io::Error::other(format!(
                "{label} 引用了未提供的技能 ${reference}"
            )));
        }
    }
    Ok(())
}

fn require_kind(path: &Path, directory: bool) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let expected = if directory {
        metadata.is_dir()
    } else {
        metadata.is_file()
    };
    if metadata.file_type().is_symlink() || !expected {
        return Err(io::Error::other(format!(
            "技能检查只接受实际目录与普通文件，不跟随符号链接: {}",
            path.display()
        )));
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.as_bytes()[0].is_ascii_lowercase()
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn route_names(text: &str) -> io::Result<BTreeSet<String>> {
    let mut section = false;
    let mut saw_heading = false;
    let mut routes = BTreeSet::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line == ROUTE_HEADING {
            if saw_heading {
                return Err(io::Error::other("根技能路由 section 重复"));
            }
            saw_heading = true;
            section = true;
            continue;
        }
        if line.starts_with("## ") {
            section = false;
        }
        if !section || line.is_empty() {
            continue;
        }
        // 路由区每个条目使用 - `$name`；解释段落不参与解析。
        if let Some(bullet) = line.strip_prefix("- ") {
            let Some(rest) = bullet.strip_prefix("`$") else {
                return Err(io::Error::other(format!(
                    "技能路由条目缺少 `$name`: {line}"
                )));
            };
            let Some((name, _)) = rest.split_once('`') else {
                return Err(io::Error::other(format!("技能路由代码跨度未闭合: {line}")));
            };
            if !valid_name(name) {
                return Err(io::Error::other(format!("非法技能路由: {name}")));
            }
            if !routes.insert(name.to_owned()) {
                return Err(io::Error::other(format!("技能路由重复: ${name}")));
            }
        }
    }
    if !saw_heading || routes.is_empty() {
        return Err(io::Error::other("根 AGENTS.md 缺少非空技能路由区"));
    }
    Ok(routes)
}

fn compare_routes(routes: &BTreeSet<String>, packages: &BTreeSet<String>) -> io::Result<()> {
    let missing: Vec<_> = routes.difference(packages).collect();
    let unrouted: Vec<_> = packages.difference(routes).collect();
    if missing.is_empty() && unrouted.is_empty() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "技能路由未闭合；缺失本地技能: {missing:?}；未列入根路由: {unrouted:?}"
    )))
}

/// 仅提取单反引号跨度中的完整小写技能名，跳过 Markdown fenced code。
fn inline_references(text: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut fence: Option<(u8, usize)> = None;
    for raw in text.lines() {
        let line = raw.trim_start();
        let marker = line.as_bytes().first().copied();
        let width = marker.map_or(0, |value| {
            line.bytes().take_while(|byte| *byte == value).count()
        });
        if let Some((value, opening_width)) = fence {
            if marker == Some(value) && width >= opening_width && line[width..].trim().is_empty() {
                fence = None;
            }
            continue;
        }
        if matches!(marker, Some(b'`' | b'~')) && width >= 3 {
            fence = Some((marker.expect("marker 已匹配"), width));
            continue;
        }
        let bytes = line.as_bytes();
        let mut cursor = 0;
        while cursor < bytes.len() {
            if bytes[cursor] != b'`' {
                cursor += 1;
                continue;
            }
            let start = cursor;
            while cursor < bytes.len() && bytes[cursor] == b'`' {
                cursor += 1;
            }
            let opening = cursor - start;
            let content = cursor;
            let mut end = None;
            while cursor < bytes.len() {
                if bytes[cursor] != b'`' {
                    cursor += 1;
                    continue;
                }
                let closing = cursor;
                while cursor < bytes.len() && bytes[cursor] == b'`' {
                    cursor += 1;
                }
                if cursor - closing == opening {
                    end = Some(closing);
                    break;
                }
            }
            if let Some(name) = end
                .filter(|_| opening == 1)
                .and_then(|end| line[content..end].strip_prefix('$'))
                .filter(|name| valid_name(name))
            {
                result.insert(name.to_owned());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "kanban-route-test-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            // create_dir 保持独占，不覆盖其他进程或上次失败的目录。
            fs::create_dir(&root).unwrap();
            let fixture = Self(root);
            fs::create_dir_all(fixture.0.join(".agents/skills/check")).unwrap();
            fs::write(
                fixture.0.join(".agents/skills/check/SKILL.md"),
                "使用 `$check`。\n",
            )
            .unwrap();
            fs::create_dir(fixture.0.join("docs")).unwrap();
            for relative in GOVERNANCE_DOCUMENTS {
                fs::write(fixture.0.join(relative), "使用 `$check`。\n").unwrap();
            }
            fixture
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn agents(body: &str) -> String {
        format!("# Repo\n{ROUTE_HEADING}\n{body}\n## 6. 文档地图\n")
    }

    #[test]
    fn local_route_and_body_reference_pass() {
        let fixture = Fixture::new();
        assert!(check(&fixture.0, &agents("- `$check`：验证。")).is_ok());
    }

    #[test]
    fn absent_local_skill_fails() {
        let fixture = Fixture::new();
        assert!(
            check(
                &fixture.0,
                &agents("- `$check`：验证。\n- `$domain-modeling`：术语。")
            )
            .is_err()
        );
    }

    #[test]
    fn missing_reference_outside_root_route_fails() {
        let fixture = Fixture::new();
        let text = format!("{}使用 `$missing`。", agents("- `$check`：验证。"));
        assert!(check(&fixture.0, &text).is_err());
    }

    #[test]
    fn duplicate_route_fails() {
        assert!(route_names(&agents("- `$check`：一。\n- `$check`：二。")).is_err());
    }

    #[test]
    fn route_requires_canonical_spelling() {
        assert!(route_names(&agents("- check：未标记。")).is_err());
    }

    #[test]
    fn path_traversal_route_fails() {
        assert!(route_names(&agents("- `$../check`：拒绝。")).is_err());
    }

    #[test]
    fn nonempty_route_section_is_required() {
        assert!(route_names("# no route").is_err());
        assert!(route_names(&agents("")).is_err());
    }

    #[test]
    fn repeated_heading_fails() {
        let text = format!(
            "{}\n{}",
            agents("- `$check`：一。"),
            agents("- `$check`：二。")
        );
        assert!(route_names(&text).is_err());
    }

    #[test]
    fn unrouted_package_fails() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.0.join(".agents/skills/docs")).unwrap();
        fs::write(fixture.0.join(".agents/skills/docs/SKILL.md"), "说明").unwrap();
        assert!(check(&fixture.0, &agents("- `$check`：验证。")).is_err());
    }

    #[test]
    fn unknown_body_reference_fails() {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join(".agents/skills/check/SKILL.md"),
            "使用 `$missing-skill`。\n",
        )
        .unwrap();
        assert!(check(&fixture.0, &agents("- `$check`：验证。")).is_err());
    }

    #[test]
    fn governance_references_report_their_owner() {
        for relative in GOVERNANCE_DOCUMENTS {
            let fixture = Fixture::new();
            fs::write(fixture.0.join(relative), "术语使用 `$missing-skill`。\n").unwrap();
            let error = check(&fixture.0, &agents("- `$check`：验证。"))
                .unwrap_err()
                .to_string();
            assert!(error.contains(relative));
            assert!(error.contains("$missing-skill"));
        }
    }

    #[test]
    fn governance_examples_do_not_become_skill_dependencies() {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join(GOVERNANCE_DOCUMENTS[0]),
            "`$HOME` `${target}` `$1` `$target_dir`\n~~~sh\n`$missing`\n~~~\n`` `$example` `` 和 `$check`。\n",
        )
        .unwrap();
        assert!(check(&fixture.0, &agents("- `$check`：验证。")).is_ok());
    }

    #[test]
    fn missing_governance_document_fails() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.0.join(GOVERNANCE_DOCUMENTS[0])).unwrap();
        assert!(check(&fixture.0, &agents("- `$check`：验证。")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_governance_document_is_rejected() {
        let fixture = Fixture::new();
        let path = fixture.0.join(GOVERNANCE_DOCUMENTS[0]);
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink("collaboration.md", path).unwrap();
        assert!(check(&fixture.0, &agents("- `$check`：验证。")).is_err());
    }

    #[test]
    fn recognizable_shell_forms_and_code_fences_are_ignored() {
        let text =
            "`$HOME` `$1` `${name}` `$name_with_underscore`\n```bash\n`$missing`\n```\n`$docs`\n";
        assert_eq!(inline_references(text), BTreeSet::from(["docs".to_owned()]));
    }

    #[test]
    fn longer_fences_and_nested_code_spans_are_ignored() {
        let text = "````markdown\n```\n`$missing`\n````\n`` `$ignored` `` `$check`\n";
        assert_eq!(
            inline_references(text),
            BTreeSet::from(["check".to_owned()])
        );
    }

    #[test]
    fn crlf_documents_are_supported() {
        assert_eq!(
            route_names(&agents("- `$check`：验证。").replace('\n', "\r\n")).unwrap(),
            BTreeSet::from(["check".to_owned()])
        );
    }

    #[test]
    fn names_follow_the_repository_subset() {
        for name in ["check", "domain-modeling", "tool2"] {
            assert!(valid_name(name));
        }
        for name in ["", "Check", "-check", "check-", "a--b", "../check", "a_b"] {
            assert!(!valid_name(name));
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlink_skill_file_is_rejected() {
        let fixture = Fixture::new();
        let path = fixture.0.join(".agents/skills/check/SKILL.md");
        fs::remove_file(&path).unwrap();
        fs::write(fixture.0.join("outside.md"), "使用 `$check`。").unwrap();
        std::os::unix::fs::symlink("../../../outside.md", &path).unwrap();
        assert!(check(&fixture.0, &agents("- `$check`：验证。")).is_err());
    }
}
