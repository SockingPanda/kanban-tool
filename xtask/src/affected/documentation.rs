use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use xtask::ToolResult;

use crate::repository::{IncludeReference, include_references, repository_files};

pub(super) fn needs_full_check(root: &Path, changed: &[String]) -> ToolResult<bool> {
    let documents: Vec<_> = changed
        .iter()
        .filter(|path| super::is_document(path))
        .collect();
    if documents.is_empty() {
        return Ok(false);
    }
    let root = fs::canonicalize(root)?;
    let documents: Vec<_> = documents
        .iter()
        .map(|path| normalize(&root.join(path)))
        .collect();
    // 删除和 rename 的旧路径仍保留在 changed 中，不能因源码中的旧 include 已移除而漏检。
    if documents
        .iter()
        .any(|path| !path.is_file() || path.is_symlink())
    {
        return Ok(true);
    }
    for source in repository_files(&root, "rs")? {
        let text = fs::read_to_string(&source)?;
        let Ok(references) = include_references(&root, &source, &text) else {
            // 无法解析的 Rust token 无法证明 include 与文档无关。
            return Ok(true);
        };
        for reference in references {
            let affected = match reference {
                IncludeReference::File(path) => documents.contains(&normalize(&path)),
                IncludeReference::Prefix(path) => {
                    let prefix = normalize(&path);
                    // concat 的最后一个静态片段也可能只是文件名的前半段。
                    documents.iter().any(|document| {
                        document
                            .as_os_str()
                            .as_encoded_bytes()
                            .starts_with(prefix.as_os_str().as_encoded_bytes())
                    })
                }
                IncludeReference::Unresolved => true,
            };
            if affected {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// 不依赖目标存在；丢失的 include 由完整 docs gate 报告。
fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "kanban-doc-impact-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            let fixture = Self(root);
            fixture.write("Cargo.toml", "[package]\nname='fixture'\nversion='0.1.0'\n");
            fixture.write("README.md", "# 普通入口\n");
            fixture.write("docs/guide.md", "# Rust 指南\n");
            fixture
        }

        fn write(&self, path: &str, content: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }

        fn full(&self, paths: &[&str]) -> bool {
            needs_full_check(
                &self.0,
                &paths
                    .iter()
                    .map(|path| (*path).to_owned())
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn included_document_and_plain_navigation_use_different_gates() {
        let fixture = Fixture::new();
        fixture.write(
            "src/lib.rs",
            "#![doc = include_str!(\"../docs/./guide.md\")]\n",
        );
        assert!(fixture.full(&["docs/guide.md"]));
        assert!(!fixture.full(&["README.md"]));
        assert!(!fixture.full(&[]));
        assert!(!fixture.full(&["src/lib.rs"]));
    }

    #[test]
    fn manifest_and_literal_concat_are_resolved() {
        let fixture = Fixture::new();
        for source in [
            "#![doc = include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/docs/\", \"guide.md\"))]",
            "#![doc = include_str!(concat!(\"../docs/\", \"guide.md\"))]",
        ] {
            fixture.write("src/lib.rs", source);
            assert!(fixture.full(&["docs/guide.md"]));
            assert!(!fixture.full(&["README.md"]));
        }
    }

    #[test]
    fn dynamic_prefix_only_upgrades_matching_documents() {
        let fixture = Fixture::new();
        fixture.write("src/lib.rs", "macro_rules! guide { ($name:literal) => { include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/docs/gui\", $name)) }; }");
        assert!(fixture.full(&["docs/guide.md"]));
        assert!(!fixture.full(&["README.md"]));
    }

    #[test]
    fn unknown_environment_freezes_the_known_prefix() {
        let fixture = Fixture::new();
        fixture.write("src/lib.rs", "const GUIDE: &str = include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/docs/\", env!(\"GUIDE\"), \".md\"));");
        assert!(fixture.full(&["docs/guide.md"]));
        assert!(!fixture.full(&["README.md"]));
    }

    #[test]
    fn unresolved_macros_and_invalid_tokens_require_full_docs() {
        let fixture = Fixture::new();
        for source in [
            "const GUIDE: &str = include_str!(env!(\"GUIDE\"));",
            "const GUIDE: &str = include_str!(concat!(env!(\"OUT_DIR\"), \"/guide.md\"));",
            "fn broken( {",
        ] {
            fixture.write("src/lib.rs", source);
            assert!(fixture.full(&["README.md"]));
        }
    }

    #[test]
    fn removed_and_renamed_documents_remain_conservative() {
        let fixture = Fixture::new();
        fs::rename(
            fixture.0.join("docs/guide.md"),
            fixture.0.join("docs/new.md"),
        )
        .unwrap();
        fixture.write("src/lib.rs", "// 原来的 include 已移除。\n");
        assert!(fixture.full(&["docs/guide.md", "docs/new.md"]));
        assert!(fixture.full(&["deleted.md"]));
    }

    #[test]
    fn comments_and_generator_strings_are_not_includes() {
        let fixture = Fixture::new();
        fixture.write("src/lib.rs", "// include_str!(env!(\"GUIDE\"));\nconst CODE: &str = r#\"include_str!(env!(\"GUIDE\"))\"#;\n");
        assert!(!fixture.full(&["README.md"]));
    }

    #[test]
    fn current_repository_distinguishes_plain_guides_and_rustdoc() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert!(!needs_full_check(root, &["docs/architecture.md".to_owned()]).unwrap());
        assert!(needs_full_check(root, &["xtask/README.md".to_owned()]).unwrap());
    }
}
