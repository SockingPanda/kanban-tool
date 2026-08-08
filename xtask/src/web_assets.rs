//! 离线 Web artifact 检查入口。
//!
//! 这个模块只负责解析仓库工具参数、解析 root-relative dist 路径并调用
//! `kanban-web-artifact` 的共享 verifier；文件清单、路径安全、bytes 和 hash 语义不在
//! `xtask` 内复制。`CARGO_PKG_VERSION` 是 workspace 发布版本，也是 host verifier 使用的
//! `serverVersion` 单一事实源。

use std::{
    env, io,
    path::{Component, Path, PathBuf},
};

use crate::ToolResult;

pub const DEFAULT_DIRECTORY: &str = "apps/web/dist";
const EXPECTED_SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
struct Options {
    root: PathBuf,
    directory: String,
}

/// 分发 `xtask web-assets check`。
pub fn run(arguments: &[String]) -> ToolResult<()> {
    let options = parse_options(arguments)?;
    let root = resolve_root(&options.root)?;
    let artifact_root = resolve_artifact_root(&root, &options.directory)?;
    let artifact = kanban_web_artifact::verify_directory(&artifact_root, EXPECTED_SERVER_VERSION)
        .map_err(|verification| {
        error(format!(
            "Web artifact 校验失败（root={}）: {verification}",
            artifact_root.display()
        ))
    })?;
    let payload_bytes = artifact.payloads().try_fold(0_u64, |total, payload| {
        total
            .checked_add(payload.descriptor().bytes)
            .ok_or_else(|| error("Web artifact payload bytes 总量溢出"))
    })?;
    println!(
        "Web artifact 校验通过: serverVersion={} buildId={} files={} bytes={} manifestSha256={} root={}",
        artifact.manifest().server_version,
        artifact.manifest().build_id,
        artifact.payloads().len(),
        payload_bytes,
        artifact.manifest_sha256(),
        artifact_root.display()
    );
    Ok(())
}

fn parse_options(arguments: &[String]) -> ToolResult<Options> {
    let mut root = PathBuf::from(".");
    let mut directory = DEFAULT_DIRECTORY.to_owned();
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--root" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| error("web-assets check 的 --root 缺少路径"))?;
                if value.starts_with('-') {
                    return Err(error("web-assets check 的 --root 缺少路径"));
                }
                root = PathBuf::from(value);
            }
            "--dir" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| error("web-assets check 的 --dir 缺少路径"))?;
                if value.starts_with('-') {
                    return Err(error("web-assets check 的 --dir 缺少路径"));
                }
                directory = value.clone();
            }
            unknown => return Err(error(format!("web-assets check 参数无效: {unknown}"))),
        }
        index += 1;
    }
    Ok(Options { root, directory })
}

fn resolve_root(raw: &Path) -> ToolResult<PathBuf> {
    let absolute = if raw.is_absolute() {
        raw.to_owned()
    } else {
        env::current_dir()?.join(raw)
    };
    let absolute = lexical_normalize_absolute(&absolute, "workspace root")?;
    Ok(absolute)
}

fn resolve_artifact_root(root: &Path, raw_directory: &str) -> ToolResult<PathBuf> {
    let relative = validate_relative_directory(raw_directory)?;
    let artifact_root = root.join(relative);
    Ok(artifact_root)
}

fn validate_relative_directory(raw: &str) -> ToolResult<&Path> {
    let path = Path::new(raw);
    if raw.is_empty() || path.is_absolute() || raw.contains('\\') {
        return Err(error(format!(
            "Web artifact --dir 必须是 root-relative regular path: {raw:?}"
        )));
    }
    if raw
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(error(format!(
            "Web artifact --dir 不得包含空、`.` 或 `..` 段: {raw:?}"
        )));
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(error(format!(
            "Web artifact --dir 只允许 root-relative regular path: {raw:?}"
        )));
    }
    Ok(path)
}

fn lexical_normalize_absolute(path: &Path, label: &str) -> ToolResult<PathBuf> {
    if !path.is_absolute() {
        return Err(error(format!(
            "{label} 必须是 absolute path: {}",
            path.display()
        )));
    }
    if path.as_os_str().to_string_lossy().starts_with("//") {
        return Err(error(format!(
            "{label} 必须只包含一个前导 slash: {}",
            path.display()
        )));
    }
    let mut normalized = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(error(format!(
                    "{label} 不得包含 parent traversal: {}",
                    path.display()
                )));
            }
            Component::Prefix(_) => {
                return Err(error(format!(
                    "{label} 不支持 path prefix: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(normalized)
}

fn error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    io::Error::other(message.into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parser_rejects_missing_flag_values_and_unknown_positionals() {
        for (values, expected) in [
            (&["--root", "--dir"][..], "--root 缺少路径"),
            (&["--dir", "--root"][..], "--dir 缺少路径"),
            (&["unexpected"][..], "参数无效"),
        ] {
            let error = parse_options(&arguments(values)).expect_err("invalid options must fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn lexical_root_rejects_double_leading_slash() {
        let error = lexical_normalize_absolute(Path::new("//tmp/workspace"), "workspace root")
            .expect_err("double-leading slash must fail closed");
        assert!(error.to_string().contains("前导 slash"));
    }
}
