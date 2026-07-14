use std::{env, path::PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("schema contract 失败: {error}");
        std::process::exit(1);
    }
}

fn run() -> kanban_schema_tool::ToolResult<()> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = arguments.first().map(String::as_str) else {
        print_usage();
        return Err(std::io::Error::other("缺少 command").into());
    };
    if command == "--help" || command == "-h" {
        print_usage();
        return Ok(());
    }

    let mut root = PathBuf::from(".");
    let mut require_closed = false;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--root" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| std::io::Error::other("--root 缺少路径"))?;
                root = PathBuf::from(value);
            }
            "--require-closed" => require_closed = true,
            unknown => {
                return Err(std::io::Error::other(format!("未知参数: {unknown}")).into());
            }
        }
        index += 1;
    }

    match command {
        "generate" => {
            kanban_schema_tool::write_generated(&root)?;
            kanban_schema_tool::check_contract(&root, require_closed)?;
            println!(
                "已生成并验证 {} 个 schema roots（未闭合项: {}）",
                kanban_contract::schema_registry().len(),
                kanban_schema_tool::unfinished_contract_count()
            );
        }
        "check" | "contract" => {
            kanban_schema_tool::check_contract(&root, require_closed)?;
            println!(
                "schema contract 已通过：{} roots，{} 未闭合项",
                kanban_contract::schema_registry().len(),
                kanban_schema_tool::unfinished_contract_count()
            );
        }
        "audit" => {
            kanban_schema_tool::audit_inventory(require_closed)?;
            println!(
                "contract/surface catalog 已通过：{} contract entries，{} surface entries，{} 未闭合项",
                kanban_contract::operation_inventory().len(),
                kanban_contract::surface_operation_catalog().len(),
                kanban_schema_tool::unfinished_contract_count()
            );
        }
        "witnesses" => {
            kanban_schema_tool::audit_inventory(false)?;
            let adopted = kanban_contract::operation_inventory()
                .iter()
                .filter(|operation| operation.migration == kanban_contract::MigrationState::Adopted)
                .collect::<Vec<_>>();
            println!("{}", serde_json::to_string_pretty(&adopted)?);
        }
        _ => {
            print_usage();
            return Err(std::io::Error::other(format!("未知 command: {command}")).into());
        }
    }
    Ok(())
}

fn print_usage() {
    println!(
        "Usage: kanban-schema <generate|check|contract|audit|witnesses> [--root PATH] [--require-closed]"
    );
}
