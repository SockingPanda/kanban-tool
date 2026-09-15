use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use xtask::ToolResult;

use crate::process::status_description;

pub(crate) fn workspace_members(root: &Path) -> ToolResult<Vec<String>> {
    #[derive(serde::Deserialize)]
    struct CargoMetadata {
        packages: Vec<CargoPackage>,
        workspace_members: Vec<String>,
    }

    #[derive(serde::Deserialize)]
    struct CargoPackage {
        id: String,
        manifest_path: String,
    }

    let root = fs::canonicalize(root)?;
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--locked",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .current_dir(&root)
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "cargo metadata 失败（{}）: {}",
            status_description(output.status),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
        .into());
    }
    let metadata = serde_json::from_slice::<CargoMetadata>(&output.stdout)
        .map_err(|error| std::io::Error::other(format!("cargo metadata 输出解析失败: {error}")))?;
    let mut members = Vec::with_capacity(metadata.workspace_members.len());
    for member_id in metadata.workspace_members {
        let package = metadata
            .packages
            .iter()
            .find(|package| package.id == member_id)
            .ok_or_else(|| {
                std::io::Error::other(format!("cargo metadata 缺少 workspace member: {member_id}"))
            })?;
        let member_root = Path::new(&package.manifest_path)
            .parent()
            .ok_or_else(|| std::io::Error::other("workspace member manifest 缺少父目录"))?;
        let relative = member_root.strip_prefix(&root).map_err(|error| {
            std::io::Error::other(format!("workspace member 不在 workspace root 下: {error}"))
        })?;
        members.push(if relative.as_os_str().is_empty() {
            ".".to_owned()
        } else {
            relative.to_string_lossy().into_owned()
        });
    }
    if members.is_empty() {
        return Err(std::io::Error::other("workspace members 为空").into());
    }
    Ok(members)
}

pub(crate) fn repository_files(root: &Path, extension: &str) -> ToolResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(root, extension, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(path: &Path, extension: &str, files: &mut Vec<PathBuf>) -> ToolResult<()> {
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

pub(crate) fn include_targets(root: &Path, source: &Path, text: &str) -> ToolResult<Vec<PathBuf>> {
    let mut targets = Vec::new();
    for invocation in syn::parse_str::<IncludeMacros>(text)?.0 {
        if let Ok(literal) = syn::parse2::<syn::LitStr>(invocation.tokens.clone()) {
            targets.push(source.parent().unwrap_or(root).join(literal.value()));
            continue;
        }
        let Ok(concat) = syn::parse2::<syn::Macro>(invocation.tokens) else {
            continue;
        };
        if !concat.path.is_ident("concat") {
            continue;
        }
        let parts = syn::parse2::<IncludeConcat>(concat.tokens)?;
        if parts.manifest
            && let Some(manifest_dir) = source
                .ancestors()
                .find(|candidate| candidate.join("Cargo.toml").is_file())
        {
            targets.push(manifest_dir.join(parts.literal.trim_start_matches('/')));
        }
    }
    Ok(targets)
}

/// 扫描 Rust token，而不是把生成器字符串、注释里的示例当作真实宏调用。
struct IncludeMacros(Vec<syn::Macro>);

impl syn::parse::Parse for IncludeMacros {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let mut found = Vec::new();
        while !input.is_empty() {
            if input.fork().parse::<syn::Macro>().is_ok() {
                let invocation = input.parse::<syn::Macro>()?;
                if invocation.path.is_ident("include_str") {
                    found.push(invocation);
                } else {
                    found.extend(syn::parse2::<Self>(invocation.tokens)?.0);
                }
            } else if input.peek(syn::token::Paren) {
                let inner;
                syn::parenthesized!(inner in input);
                found.extend(inner.parse::<Self>()?.0);
            } else if input.peek(syn::token::Brace) {
                let inner;
                syn::braced!(inner in input);
                found.extend(inner.parse::<Self>()?.0);
            } else if input.peek(syn::token::Bracket) {
                let inner;
                syn::bracketed!(inner in input);
                found.extend(inner.parse::<Self>()?.0);
            } else {
                input.step(|cursor| {
                    let (_, next) = cursor.token_tree().expect("非空 token stream");
                    Ok(((), next))
                })?;
            }
        }
        Ok(Self(found))
    }
}

#[derive(Default)]
struct IncludeConcat {
    manifest: bool,
    literal: String,
}

impl syn::parse::Parse for IncludeConcat {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let mut result = Self::default();
        let mut dynamic = false;
        while !input.is_empty() {
            if !dynamic && input.peek(syn::LitStr) {
                result
                    .literal
                    .push_str(&input.parse::<syn::LitStr>()?.value());
            } else if !dynamic && input.fork().parse::<syn::Macro>().is_ok() {
                let invocation = input.parse::<syn::Macro>()?;
                result.manifest |= invocation.path.is_ident("env")
                    && syn::parse2::<syn::LitStr>(invocation.tokens)
                        .is_ok_and(|name| name.value() == "CARGO_MANIFEST_DIR");
            } else if input.peek(syn::Token![,]) {
                input.parse::<syn::Token![,]>()?;
            } else {
                // 宏定义中的 $literal 由展开和编译检查；这里只核对静态前缀。
                dynamic = true;
                input.step(|cursor| {
                    let (_, next) = cursor.token_tree().expect("非空 token stream");
                    Ok(((), next))
                })?;
            }
        }
        Ok(result)
    }
}

pub(crate) fn same_file(left: &Path, right: &Path) -> bool {
    fs::canonicalize(left).ok() == fs::canonicalize(right).ok()
}

pub(crate) fn required_directory(path: PathBuf, label: &str) -> ToolResult<PathBuf> {
    if path.is_symlink() || !path.is_dir() {
        return Err(std::io::Error::other(format!(
            "{label} 不存在或不是普通目录: {}",
            path.display()
        ))
        .into());
    }
    Ok(path)
}

pub(crate) fn ensure_regular_directory(path: &Path, label: &str) -> ToolResult<()> {
    if path.is_symlink() || !path.is_dir() {
        return Err(std::io::Error::other(format!(
            "{label} 不存在或不是普通目录: {}",
            path.display()
        ))
        .into());
    }
    Ok(())
}

pub(crate) fn ensure_regular_file(path: &Path, label: &str) -> ToolResult<()> {
    if path.is_symlink() || !path.is_file() {
        return Err(std::io::Error::other(format!(
            "{label} 不存在或不是普通文件: {}",
            path.display()
        ))
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, time::SystemTime};

    #[test]
    fn include_targets_ignore_generator_literals_and_comments() {
        let text = r##"
            // include_str!("missing-comment.md")
            const TEMPLATE: &str = "let value = include_str!(\"missing-template.md\");";
            const RAW: &str = r#"include_str!("missing-raw.md")"#;
            #[doc = include_str!("../README.md")]
            fn body() { let _ = include_str! ("sample.txt"); }
        "##;
        let targets = include_targets(Path::new("/repo"), Path::new("/repo/src/lib.rs"), text)
            .expect("Rust tokens should parse");
        assert_eq!(
            targets,
            [
                Path::new("/repo/src/../README.md"),
                Path::new("/repo/src/sample.txt")
            ]
        );
    }

    #[test]
    fn include_targets_resolve_all_static_concat_segments() {
        let root = temp_root("include-concat");
        fs::write(root.join("Cargo.toml"), "").unwrap();
        let text = r#"include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/", "guide.md"));"#;
        assert_eq!(
            include_targets(&root, &root.join("src/lib.rs"), text).unwrap(),
            [root.join("docs/guide.md")]
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("xtask-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("temporary root should be creatable");
        path
    }

    #[test]
    fn workspace_members_reads_the_workspace_table_with_cargo_metadata() {
        let root = temp_root("workspace-members");
        fs::write(
            root.join("Cargo.toml"),
            r#"
[workspace.metadata]
members = ["not/a/workspace/member"]

[workspace]
members = [
    "crates/one", # inline comments are valid TOML
    "apps/two",
]
"#,
        )
        .expect("workspace manifest should be writable");
        for member in ["crates/one", "apps/two"] {
            let member_root = root.join(member);
            fs::create_dir_all(member_root.join("src"))
                .expect("workspace fixture directory should be creatable");
            fs::write(
                member_root.join("Cargo.toml"),
                format!(
                    "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
                    member.replace('/', "-")
                ),
            )
            .expect("workspace fixture manifest should be writable");
            fs::write(member_root.join("src/lib.rs"), "")
                .expect("workspace source should be writable");
        }

        let lockfile_status = Command::new("cargo")
            .args(["generate-lockfile", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .current_dir(&root)
            .status()
            .expect("cargo generate-lockfile should run");
        assert!(
            lockfile_status.success(),
            "workspace fixture lockfile should be generated"
        );

        assert_eq!(
            workspace_members(&root).expect("workspace members should parse"),
            vec!["crates/one", "apps/two"]
        );

        fs::remove_dir_all(root).expect("temporary root should be removable");
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
}
