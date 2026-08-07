use std::{fs, path::Path};

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
    check_markdown_links(root)?;
    check_include_str_targets(root)?;
    check_crate_readme_includes(root, &agents_text)?;
    check_adr_index(root)?;
    println!("ok: 文档链接、include_str!、crate README、ADR index 和 workspace crate map 已通过");
    Ok(())
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
