use std::{
    env,
    path::{Path, PathBuf},
};

use xtask::ToolResult;

use crate::{
    affected,
    check::{agents, dependencies, docs, tooling},
};

pub(crate) fn run() -> ToolResult<()> {
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
    if group == "affected" {
        return match subcommand {
            Some(command) => run_affected(command, &arguments[2..]),
            None => invalid("affected 缺少子命令"),
        };
    }
    let (root, options) = parse_options(&arguments[2..])?;

    match (group, subcommand) {
        ("schema", Some(command)) => run_schema(command, &root, options.require_closed),
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

#[derive(Default)]
struct Options {
    require_closed: bool,
}

fn parse_options(arguments: &[String]) -> ToolResult<(PathBuf, Options)> {
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

fn run_schema(command: &str, root: &Path, require_closed: bool) -> ToolResult<()> {
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

fn invalid(message: impl Into<String>) -> ToolResult<()> {
    print_usage();
    Err(std::io::Error::other(message.into()).into())
}

fn print_usage() {
    println!(
        "用法：xtask <affected plan|json|run|self-test|docs check|schema generate|check|audit|witnesses|deps check|agents check|tooling check> [--base REF] [--root PATH] [--require-closed]"
    );
}
