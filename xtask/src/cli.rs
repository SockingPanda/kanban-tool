use std::{
    env,
    path::{Path, PathBuf},
};

use xtask::ToolResult;

use crate::{
    affected,
    check::{agents, dependencies, docs, tooling},
    package,
};

pub(crate) fn run() -> ToolResult<()> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.first().map(String::as_str) == Some("package") {
        let subcommand = arguments.get(1).map(String::as_str);
        return match subcommand {
            Some("cli") => package::run(&arguments[2..]),
            None => invalid("package 缺少子命令"),
            Some(command) => invalid(format!("package 不支持子命令: {command}")),
        };
    }
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
    if group == "affected" {
        return match subcommand {
            Some(command) => run_affected(command, &arguments[2..]),
            None => invalid("affected 缺少子命令"),
        };
    }
    let root = parse_options(&arguments[2..])?;

    match (group, subcommand) {
        ("schema", Some(command)) => run_schema(command, &root),
        ("docs", Some("check")) => docs::run(&root),
        ("deps", Some("check")) => dependencies::run(&root),
        ("agents", Some("check")) => agents::run(&root),
        ("tooling", Some("check")) => tooling::run(&root),
        ("schema", None) => invalid("schema 缺少子命令"),
        ("docs", Some(command)) => invalid(format!("docs 不支持子命令: {command}")),
        ("docs", None) => invalid("docs 缺少子命令"),
        ("deps", Some(command)) => invalid(format!("deps 不支持子命令: {command}")),
        ("agents", Some(command)) => invalid(format!("agents 不支持子命令: {command}")),
        ("tooling", Some(command)) => invalid(format!("tooling 不支持子命令: {command}")),
        ("tooling", None) => invalid("tooling 缺少子命令"),
        (group, Some(command)) => invalid(format!("未知 command: {group} {command}")),
        (group, None) => invalid(format!("未知 command: {group}")),
    }
}

fn run_affected(command: &str, arguments: &[String]) -> ToolResult<()> {
    let mut root = PathBuf::from(".");
    let mut base = "main".to_owned();
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        if argument == "--root" {
            index += 1;
            let value = arguments
                .get(index)
                .ok_or_else(|| std::io::Error::other("--root 缺少路径"))?;
            root = PathBuf::from(value);
        } else if argument == "--base" {
            index += 1;
            base = arguments
                .get(index)
                .ok_or_else(|| std::io::Error::other("--base 缺少引用"))?
                .clone();
        } else if let Some(value) = argument.strip_prefix("--base=") {
            base = value.to_owned();
        } else if argument.starts_with("base=") {
            base = argument.clone();
        } else {
            return invalid(format!("affected 参数无效: {argument}"));
        }
        index += 1;
    }
    affected::run(&root, command, &base)
}

fn parse_options(arguments: &[String]) -> ToolResult<PathBuf> {
    let mut root = PathBuf::from(".");
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
            unknown => return Err(std::io::Error::other(format!("未知参数: {unknown}")).into()),
        }
        index += 1;
    }
    Ok(root)
}

fn run_schema(command: &str, root: &Path) -> ToolResult<()> {
    match command {
        "generate" => {
            xtask::write_generated(root)?;
            xtask::check_contract(root)?;
            println!(
                "已生成并验证 {} 个 schema roots",
                kanban_protocol::schema_registry().len()
            );
            Ok(())
        }
        "check" => {
            xtask::check_contract(root)?;
            println!(
                "schema contract 已通过：{} roots",
                kanban_protocol::schema_registry().len()
            );
            Ok(())
        }
        "audit" => {
            xtask::audit_inventory()?;
            println!(
                "contract/surface catalog 已通过：{} contract entries，{} surface entries",
                kanban_protocol::operation_inventory().len(),
                kanban_protocol::surface_operation_catalog().len()
            );
            Ok(())
        }
        other => invalid(format!("未知 schema command: {other}")),
    }
}

fn invalid(message: impl Into<String>) -> ToolResult<()> {
    print_usage();
    Err(std::io::Error::other(message.into()).into())
}

fn print_usage() {
    println!(
        "用法：xtask <affected plan|json|run|self-test|docs check|schema generate|check|audit|deps check|agents check|tooling check|package cli> [--base REF] [--root PATH]"
    );
}
