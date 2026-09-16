//! 正式 RPC 契约生成：解析 DTO 的 Rust 类型，不执行业务或访问数据库。

mod artifacts;
mod emit;
mod model;
mod web;

use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

use crate::ToolResult;
use emit::{Emitter, ident};
use model::{Field, Model, Ty};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Operation {
    id: String,
    method: String,
    path: Option<String>,
    query: Option<String>,
    input: Option<String>,
    response: String,
}

/// 生成或检查 protocol owner 的正式 .proto、codec 和 RPC parts 清单。
pub fn run(root: &Path, check: bool) -> ToolResult<()> {
    let owner = root.join("crates/kanban-protocol");
    let operations: Vec<Operation> =
        serde_json::from_slice(&fs::read(owner.join("proto/rpc-operations.json"))?)?;
    let model = Model::read(&owner)?;
    let mut emitter = Emitter::new(&model);
    let mut requests = String::new();
    let mut request_codecs = String::new();
    let mut methods = String::new();
    let mut manifest = Vec::new();
    let mut names = BTreeSet::new();
    for operation in &operations {
        if !names.insert(operation.method.clone()) {
            return Err(format!("重复 RPC {}", operation.method).into());
        }
        let req = format!("{}Request", operation.method);
        let reply = format!("{}Response", operation.method);
        let mut fields = Vec::new();
        let mut used = BTreeSet::new();
        let mut part_types = Vec::new();
        let mut to = String::new();
        let mut from = Vec::new();
        let mut mapping = Vec::new();
        for (name, part) in [
            ("path", &operation.path),
            ("query", &operation.query),
            ("input", &operation.input),
        ] {
            let Some(part) = part else {
                part_types.push("()".into());
                from.push("()".into());
                continue;
            };
            let ty = model.parse(part)?;
            part_types.push(ty.rust());
            let declared = model.fields(&ty)?;
            let nested = declared.iter().any(|f| used.contains(&f.name));
            if nested {
                let nested_name = if name == "query" { "options" } else { name };
                let field = Field {
                    name: nested_name.into(),
                    ty: ty.clone(),
                    default: None,
                };
                to.push_str(&format!(
                    "{nested_name}:{},",
                    emitter.encode_field(&field, name)?
                ));
                from.push(emitter.decode_field(&field, &format!("self.{nested_name}"))?);
                mapping.push(
                    serde_json::json!({"part":name,"dto":part,"field":nested_name,"nested":true}),
                );
                fields.push(field);
                used.insert(nested_name.to_owned());
            } else {
                let mut decoded = String::new();
                for field in &declared {
                    used.insert(field.name.clone());
                    to.push_str(&format!(
                        "{}:{},",
                        ident(&field.name),
                        emitter.encode_field(field, &format!("{name}.{}", ident(&field.name)))?
                    ));
                    decoded.push_str(&format!(
                        "{}:{},",
                        ident(&field.name),
                        emitter.decode_field(field, &format!("self.{}", ident(&field.name)))?
                    ));
                    mapping.push(serde_json::json!({"part":name,"dto":part,"field":field.name,"nested":false,"rust_type":field.ty.rust()}));
                }
                let Ty::Named(base, _) = &ty else {
                    return Err("parts 必须是具名结构".into());
                };
                from.push(format!("{base}{{{decoded}}}"));
                fields.extend(declared);
            }
        }
        let body = fields
            .iter()
            .enumerate()
            .map(|(i, f)| emitter.field_line(f, i + 1))
            .collect::<ToolResult<Vec<_>>>()?
            .join("\n");
        requests.push_str(&format!(
            "// {}\nmessage {req} {{\n{body}\n}}\n",
            operation.id
        ));
        let signature = part_types
            .iter()
            .zip(["path", "query", "input"])
            .map(|(ty, name)| {
                if ty == "()" {
                    format!("_{name}:{ty}")
                } else {
                    format!("{name}:{ty}")
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        let args = part_types.join(",");
        request_codecs.push_str(&format!("impl v1::{req} {{ pub fn decode_parts(self)->Result<({args}),RpcCodecError>{{Ok(({}))}} pub fn from_parts({signature})->Result<Self,RpcCodecError>{{Ok(Self{{{to}}})}} }}\n",from.join(",")));
        let ty = model.parse(&operation.response)?;
        let reply_fields = model.fields(&ty)?;
        emitter.emit_struct(&reply, &ty, &reply_fields)?;
        methods.push_str(&format!(
            "  rpc {}({req}) returns ({reply});\n",
            operation.method
        ));
        manifest.push(serde_json::json!({"operation_id":operation.id,"method":operation.method,"request":req,"response":reply,"path_dto":operation.path,"query_dto":operation.query,"input_dto":operation.input,"response_dto":operation.response,"resolved_parts":part_types,"resolved_response":ty.rust(),"fields":mapping}));
    }
    emitter.ensure(&model.parse("crate::ApiErrorCode")?)?;
    let dto = format!(
        "// @generated: xtask rpc-codegen；字段编号属于正式契约，不得重排。\nsyntax = \"proto3\";\npackage kanban.v1;\nimport \"kanban/v1/common.proto\";\n\n{}",
        emitter
            .messages
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    let service = format!(
        "// @generated: xtask rpc-codegen\nsyntax = \"proto3\";\npackage kanban.v1;\nimport \"kanban/v1/common.proto\";\nimport \"kanban/v1/dto.proto\";\n\nservice KanbanService {{\n{methods}}}\n\nmessage ErrorDetail {{\n  DtoApiErrorCode code = 1;\n  string message = 2;\n  bool retryable = 3;\n}}\n\n{requests}"
    );
    let codecs = format!(
        "// @generated: xtask rpc-codegen；只进行字段级 typed 转换。\nuse super::{{v1, RpcCodecError}};\nuse super::codec::{{required,finite,encode_json,decode_json,validate_stream_event}};\n{request_codecs}\n{}",
        emitter
            .codecs
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    let parsed = syn::parse_file(&codecs).map_err(|e| {
        let _ = fs::write("/tmp/kanban-rpc-codecs-invalid.rs", &codecs);
        format!("生成 codec Rust 语法错误: {e}")
    })?;
    let ledger_path = owner.join("proto/rpc-field-numbers.json");
    let mut ledger: artifacts::FieldNumbers = if ledger_path.exists() {
        serde_json::from_slice(&fs::read(&ledger_path)?)?
    } else {
        Default::default()
    };
    let dto = artifacts::stabilize(&dto, &mut ledger)?;
    let service = artifacts::stabilize(&service, &mut ledger)?;
    write(&owner.join("proto/kanban/v1/dto.proto"), &dto, check)?;
    write(&owner.join("proto/kanban/v1/kanban.proto"), &service, check)?;
    write(
        &ledger_path,
        &(serde_json::to_string_pretty(&ledger)? + "\n"),
        check,
    )?;
    write(
        &owner.join("src/rpc/generated.rs"),
        &format_rust(&prettyplease::unparse(&parsed))?,
        check,
    )?;
    write(
        &owner.join("proto/rpc-methods.json"),
        &(serde_json::to_string_pretty(&manifest)? + "\n"),
        check,
    )?;
    artifacts::fixture_tests(&owner, &operations, check)?;
    web::run(root, &model, &operations, check)?;
    println!(
        "RPC 契约：{} 个具名方法，{} 个 DTO/集合/patch 声明；{}",
        operations.len(),
        emitter.messages.len(),
        if check {
            "生成内容一致"
        } else {
            "已生成"
        }
    );
    Ok(())
}

fn write(path: &Path, content: &str, check: bool) -> ToolResult<()> {
    if fs::read_to_string(path).is_ok_and(|actual| actual == content) {
        return Ok(());
    }
    if check {
        return Err(format!("生成 artifact 过期: {}", path.display()).into());
    } else {
        fs::write(path, content)?;
    }
    Ok(())
}

fn format_rust(source: &str) -> ToolResult<String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("rustfmt stdin 不可用")?
        .write_all(source.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!("rustfmt 失败：{}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}
