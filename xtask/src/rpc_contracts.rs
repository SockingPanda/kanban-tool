//! 从同一 Protobuf source 生成并核对浏览器客户端。

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::ToolResult;

const WEB_PACKAGE: &str = "apps/web";

pub fn run(root: &Path, action: &str) -> ToolResult<()> {
    if !matches!(action, "generate" | "check") {
        return Err(std::io::Error::other(format!("rpc-contracts 不支持 {action}")).into());
    }
    let root = root.canonicalize()?;
    crate::rpc_codegen::run(&root, action == "check")?;
    generate(
        &root,
        action,
        "crates/kanban-protocol/proto",
        "apps/web/src/generated/rpc",
    )
}

fn generate(root: &Path, action: &str, source: &str, output: &str) -> ToolResult<()> {
    let staging = GenerationDirectory::new()?;
    let proto_root = root.join(source);
    let sources = files(&proto_root)?;
    let protos: Vec<_> = sources
        .keys()
        .filter(|p| p.extension().is_some_and(|e| e == "proto"))
        .collect();
    if protos.is_empty() {
        return Err(std::io::Error::other("Protobuf source 为空").into());
    }
    let compiler = std::env::var_os("PROTOC").unwrap_or_else(|| "protoc".into());
    let mut command = Command::new(compiler);
    command
        .current_dir(&proto_root)
        .arg("-I")
        .arg(&proto_root)
        .arg(format!(
            "--plugin=protoc-gen-es={}",
            root.join(WEB_PACKAGE)
                .join("node_modules/.bin/protoc-gen-es")
                .display()
        ))
        .arg(format!("--es_out={}", staging.path.display()))
        .arg("--es_opt=target=ts");
    for source in &protos {
        command.arg(source);
    }
    let status = command.status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!("Protobuf-ES 生成失败: {status}")).into());
    }
    let mut expected = files(&staging.path)?;
    // Protobuf-ES 会输出额外的末尾空行；提交产物统一保留一个换行。
    for bytes in expected.values_mut() {
        while bytes.ends_with(b"\n\n") {
            bytes.pop();
        }
    }
    let destination = root.join(output);
    let actual = if destination.exists() {
        files(&destination)?
    } else {
        BTreeMap::new()
    };
    if action == "check" {
        if actual != expected {
            return Err(std::io::Error::other(
                "RPC 客户端与 proto 不一致；运行 just grpc-contracts-generate",
            )
            .into());
        }
    } else {
        for (relative, bytes) in &expected {
            let target = destination.join(relative);
            fs::create_dir_all(
                target
                    .parent()
                    .ok_or_else(|| std::io::Error::other("生成路径没有父目录"))?,
            )?;
            fs::write(target, bytes)?;
        }
        for relative in actual.keys().filter(|p| !expected.contains_key(*p)) {
            fs::remove_file(destination.join(relative))?;
        }
    }
    println!(
        "RPC 客户端 {action} 通过：{} 个 proto，{} 个生成文件",
        protos.len(),
        expected.len()
    );
    Ok(())
}

fn files(root: &Path) -> ToolResult<BTreeMap<PathBuf, Vec<u8>>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() || (!kind.is_file() && !kind.is_dir()) {
                return Err(std::io::Error::other(format!(
                    "RPC 生成树包含非普通文件: {}",
                    entry.path().display()
                ))
                .into());
            }
            if kind.is_dir() {
                pending.push(entry.path());
            } else {
                result.insert(
                    entry.path().strip_prefix(root)?.to_path_buf(),
                    fs::read(entry.path())?,
                );
            }
        }
    }
    Ok(result)
}

struct GenerationDirectory {
    path: PathBuf,
}

impl GenerationDirectory {
    fn new() -> ToolResult<Self> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path =
            std::env::temp_dir().join(format!("kanban-rpc-codegen-{}-{nonce}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for GenerationDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
