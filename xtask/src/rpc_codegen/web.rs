//! 浏览器字段 codec：保留 serde 业务形状，生产调用逐项绑定正式 Connect 方法。

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use super::{
    Operation,
    model::{Field, Model, Ty, snake},
    write,
};
use crate::ToolResult;

pub fn run(root: &Path, model: &Model, operations: &[Operation], check: bool) -> ToolResult<()> {
    let destination = root.join("apps/web/src/lib/rpc");
    if !check {
        fs::create_dir_all(&destination)?;
    }
    let mut emitter = WebEmitter {
        model,
        seen: BTreeSet::new(),
        functions: BTreeMap::new(),
    };
    let mut requests = String::new();
    let mut calls = String::new();
    let mut decode_requests = String::new();
    let mut encode_responses = String::new();
    for operation in operations {
        let method = &operation.method;
        let mut body = String::new();
        let mut setup = String::new();
        let mut decoded_parts = String::new();
        let mut used = BTreeSet::new();
        for (part, ty) in [
            ("path", &operation.path),
            ("query", &operation.query),
            ("input", &operation.input),
        ] {
            let Some(ty) = ty else { continue };
            let ty = model.parse(ty)?;
            let fields = model.fields(&ty)?;
            if fields.iter().any(|field| used.contains(&field.name)) {
                return Err(format!("Web RPC {method} 出现需要明确接口的 parts 字段冲突").into());
            }
            let keys = fields
                .iter()
                .map(|field| model.json_field(&ty, &field.name).0)
                .collect::<Vec<_>>();
            setup.push_str(&format!(
                "  const {part} = c.record(call.{part} ?? {{}}, {})\n",
                serde_json::to_string(&keys)?
            ));
            let mut decoded = String::new();
            for field in fields {
                let (json, skip) = model.json_field(&ty, &field.name);
                let expr = emitter.encode_field(&field, &format!("{part}[{json:?}]"))?;
                body.push_str(&format!("    {}: {expr},\n", camel(&field.name)));
                let target = format!("wire.{}", camel(&field.name));
                let mut converted = emitter.decode_field(&field, &target, skip.as_deref())?;
                if field.default.is_some()
                    && !matches!(field.ty, Ty::List(_) | Ty::Map(_, _) | Ty::Option(_))
                    && !json_body(&field.ty)
                {
                    converted = format!("{target} === undefined ? undefined : {converted}");
                }
                decoded.push_str(&format!("{json:?}: {converted},"));
                used.insert(field.name);
            }
            decoded_parts.push_str(&format!("{part}: c.omitUndefined({{{decoded}}}),"));
        }
        let call_arg = "call";
        if setup.is_empty() {
            setup.push_str("  void call\n");
        }
        requests.push_str(&format!("export function encode{method}Request({call_arg}: RpcCall): s.{method}Request {{\n{setup}  return create(s.{method}RequestSchema, {{\n{body}  }})\n}}\n\n"));
        let response = model.parse(&operation.response)?;
        emitter.structure(
            &format!("{method}Response"),
            &response,
            &model.fields(&response)?,
            "d",
        )?;
        let client_method = method[..1].to_lowercase() + &method[1..];
        calls.push_str(&format!("    case {method:?}: return decode{method}Response(await client.{client_method}(encode{method}Request(call), options))\n"));
        decode_requests.push_str(&format!("    case {method:?}: {{ const wire = fromBinary(s.{method}RequestSchema, bytes); {}return {{{decoded_parts}}} }}\n", if decoded_parts.is_empty() { "void wire; " } else { "" }));
        encode_responses.push_str(&format!("    case {method:?}: return toBinary(d.{method}ResponseSchema, encode{method}Response(payload))\n"));
    }
    emitter.ensure(&model.parse("crate::ApiErrorCode")?)?;
    let source = format!(
        "// @generated: xtask rpc-codegen；显式字段 codec 与具名 Connect 调用。\nimport {{ create, fromBinary, toBinary }} from \"@bufbuild/protobuf\"\nimport type {{ Client, CallOptions }} from \"@connectrpc/connect\"\nimport type {{ RpcCall, RpcMethod }} from \"../../application/data/rpc-transport\"\nimport * as s from \"../../generated/rpc/kanban/v1/kanban_pb\"\nimport * as d from \"../../generated/rpc/kanban/v1/dto_pb\"\nimport {{ EmptySchema }} from \"../../generated/rpc/kanban/v1/common_pb\"\nimport * as c from \"./value-codec\"\nimport {{ eventCase, validateEventCase, validateStreamEvent }} from \"./event-shape\"\n\n{requests}{}\nexport async function invokeRpc(client: Client<typeof s.KanbanService>, call: RpcCall, options: CallOptions): Promise<unknown> {{\n  switch (call.method) {{\n{calls}  }}\n}}\n\n/** 用同一份字段映射生成真实 binary RPC 测试响应。 */\nexport function encodeRpcResponse(method: RpcMethod, payload: unknown): Uint8Array {{\n  switch (method) {{\n{encode_responses}  }}\n}}\n\n/** 解码请求以供 transport fixture 核对业务 parts，不解析 HTTP URL。 */\nexport function decodeRpcRequest(method: RpcMethod, bytes: Uint8Array): Pick<RpcCall, \"path\" | \"query\" | \"input\"> {{\n  switch (method) {{\n{decode_requests}  }}\n}}\n",
        emitter
            .functions
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    write(&destination.join("codec.generated.ts"), &source, check)?;
    let names = operations
        .iter()
        .map(|operation| format!("  | {:?}", operation.method))
        .collect::<Vec<_>>()
        .join("\n");
    write(
        &destination.join("methods.generated.ts"),
        &format!(
            "// @generated: xtask rpc-codegen；纯方法名契约，不依赖 Protobuf 或 Connect。\nexport type RpcMethod =\n{names}\n"
        ),
        check,
    )?;
    fixtures(root, operations, check)?;
    Ok(())
}

struct WebEmitter<'a> {
    model: &'a Model,
    seen: BTreeSet<String>,
    functions: BTreeMap<String, String>,
}

fn camel(value: &str) -> String {
    let mut pieces = value.split('_');
    let mut result = pieces.next().unwrap_or_default().to_owned();
    for piece in pieces {
        let mut chars = piece.chars();
        if let Some(first) = chars.next() {
            result.extend(first.to_uppercase());
            result.push_str(chars.as_str());
        }
    }
    result
}

fn json_body(ty: &Ty) -> bool {
    matches!(ty, Ty::Named(name, _) if name.ends_with("::JsonBodyFieldWire"))
}
fn event_payload(ty: &Ty) -> bool {
    matches!(ty, Ty::Named(name, _) if name.ends_with("::EventPayload"))
}

impl WebEmitter<'_> {
    fn ensure(&mut self, ty: &Ty) -> ToolResult<()> {
        if matches!(ty, Ty::Scalar(_) | Ty::Json | Ty::Unit) || json_body(ty) {
            return Ok(());
        }
        let name = self.model.proto_name(ty);
        if !self.seen.insert(name.clone()) {
            return Ok(());
        }
        match ty {
            Ty::Named(_, _) if self.model.is_enum(ty) => self.enumeration(&name, ty)?,
            Ty::Named(_, _) if self.model.is_tuple(ty) => {
                let field = self.model.fields(ty)?.remove(0);
                let validator = match name.as_str() {
                    "DtoApiTaskPriority" => Some("taskPriority"),
                    "DtoUnitInterval" => Some("unitInterval"),
                    "DtoPositiveRank" => Some("positiveRank"),
                    "DtoTaskReadLabel" => Some("taskReadLabel"),
                    _ => None,
                };
                let encode = self.encode_field(&field, "value")?;
                let mut decode = self.decode_field(&field, "wire.value", None)?;
                let setup = validator
                    .map(|validate| format!("  value = c.{validate}(value)\n"))
                    .unwrap_or_default();
                if let Some(validate) = validator {
                    decode = format!("c.{validate}({decode})");
                }
                self.functions.insert(name.clone(), format!("export function encode{name}(value: unknown): d.{name} {{\n{setup}  return create(d.{name}Schema, {{ value: {encode} }})\n}}\nexport function decode{name}(wire: d.{name}): unknown {{\n  return {decode}\n}}\n"));
            }
            Ty::Named(_, _) => self.structure(&name, ty, &self.model.fields(ty)?, "d")?,
            Ty::List(_) | Ty::Map(_, _) | Ty::Option(_) => {
                let field_name = match ty {
                    Ty::List(_) => "items",
                    Ty::Map(_, _) => "entries",
                    _ => "value",
                };
                let field = Field {
                    name: field_name.into(),
                    ty: ty.clone(),
                    default: None,
                };
                let encode = self.encode_field(&field, "value")?;
                let decode = self.decode_field(&field, &format!("wire.{field_name}"), None)?;
                self.functions.insert(name.clone(), format!("export function encode{name}(value: unknown): d.{name} {{\n  return create(d.{name}Schema, {{ {field_name}: {encode} }})\n}}\nexport function decode{name}(wire: d.{name}): unknown {{\n  return {decode}\n}}\n"));
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn structure(&mut self, name: &str, ty: &Ty, fields: &[Field], module: &str) -> ToolResult<()> {
        let mut encode = String::new();
        let mut decode = String::new();
        for field in fields {
            let (json, skip) = self.model.json_field(ty, &field.name);
            let source = format!("dto[{json:?}]");
            let target = format!("wire.{}", camel(&field.name));
            let (to, from) = if event_payload(&field.ty) {
                self.ensure(&field.ty)?;
                (
                    format!("encodeDtoEventPayload({source}, c.text(dto.kind))"),
                    format!("decodeDtoEventPayload(c.required({target}), c.required(wire.kind))"),
                )
            } else {
                (
                    self.encode_field(field, &source)?,
                    self.decode_field(field, &target, skip.as_deref())?,
                )
            };
            encode.push_str(&format!("    {}: {to},\n", camel(&field.name)));
            decode.push_str(&format!("    {json:?}: {from},\n"));
        }
        let keys = fields
            .iter()
            .map(|field| self.model.json_field(ty, &field.name).0)
            .collect::<Vec<_>>();
        let (arg, mut setup, wire) = if fields.is_empty() {
            ("value", "  c.record(value, [])\n".into(), "_wire")
        } else {
            (
                "value",
                format!(
                    "  const dto = c.record(value, {})\n",
                    serde_json::to_string(&keys)?
                ),
                "wire",
            )
        };
        let mut decode_body = format!("  return c.omitUndefined({{\n{decode}  }})\n");
        if fields.is_empty() {
            decode_body.insert_str(0, "  void _wire\n");
        }
        if ty.rust() == "crate::sse::StreamEventData" {
            setup.push_str("  validateStreamEvent(dto)\n");
            decode_body = format!(
                "  const value = c.omitUndefined({{\n{decode}  }})\n  validateStreamEvent(value)\n  return value\n"
            );
        }
        self.functions.insert(name.into(), format!("export function encode{name}({arg}: unknown): {module}.{name} {{\n{setup}  return create({module}.{name}Schema, {{\n{encode}  }})\n}}\nexport function decode{name}({wire}: {module}.{name}): Record<string, unknown> {{\n{decode_body}}}\n"));
        Ok(())
    }

    fn enumeration(&mut self, name: &str, ty: &Ty) -> ToolResult<()> {
        let variants = self.model.variants(ty)?;
        if variants.iter().all(|(_, fields, _)| fields.is_empty()) {
            let names = variants
                .iter()
                .map(|(variant, _, _)| {
                    format!(
                        "  {:?}: d.{name}.{},",
                        self.model.json_variant(ty, variant),
                        snake(variant).to_uppercase()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            self.functions.insert(name.into(), format!("const {name}Names = {{\n{names}\n}} as const\nexport function encode{name}(value: unknown): d.{name} {{ return c.enumValue(value, {name}Names) }}\nexport function decode{name}(value: d.{name}): string {{ return c.enumName(value, {name}Names) }}\n"));
            return Ok(());
        }
        if !event_payload(ty) {
            return Err(format!("Web codec 尚未明确 enum {} 的 serde 表示", ty.rust()).into());
        }
        let mut encode = String::new();
        let mut decode = String::new();
        for (variant, fields, _) in variants {
            let field = &fields[0];
            let case = camel(&snake(&variant));
            let to = self.encode_element(&field.ty, "value")?;
            let from = self.decode_element(&field.ty, "wire.value.value")?;
            encode.push_str(&format!("    case {case:?}: return create(d.{name}Schema, {{ value: {{ case: {case:?}, value: {to} }} }})\n"));
            decode.push_str(&format!("    case {case:?}: value = {from}; break\n"));
        }
        self.functions.insert(name.into(), format!("export function encode{name}(value: unknown, kind: string): d.{name} {{\n  switch (eventCase(kind, value)) {{\n{encode}  }}\n}}\nexport function decode{name}(wire: d.{name}, kind: string): unknown {{\n  let value: unknown\n  switch (wire.value.case) {{\n{decode}    default: throw new c.RpcCodecError(\"RPC event payload 缺少分支。\")\n  }}\n  validateEventCase(kind, wire.value.case, value)\n  return value\n}}\n"));
        Ok(())
    }

    fn encode_field(&mut self, field: &Field, source: &str) -> ToolResult<String> {
        if json_body(&field.ty) {
            return Ok(format!("c.present({source}, c.encodeJson)"));
        }
        match &field.ty {
            Ty::Option(inner) => match &**inner {
                Ty::Option(value) => {
                    let name = format!("Patch{}", self.model.proto_name(value));
                    let convert = self.encode_element(value, "value")?;
                    Ok(format!(
                        "c.present({source}, (value) => create(d.{name}Schema, {{ change: value === null ? {{ case: \"clear\", value: create(EmptySchema) }} : {{ case: \"value\", value: {convert} }} }}))"
                    ))
                }
                _ => {
                    let convert = self.encode_element(inner, "value")?;
                    Ok(format!("c.optional({source}, (value) => {convert})"))
                }
            },
            _ => {
                let convert = self.encode_value(&field.ty, "value")?;
                if field.default.is_some() {
                    if matches!(&field.ty, Ty::List(inner) if **inner == Ty::Scalar("u8".into())) {
                        return Ok(format!("c.bytes({source} ?? new Uint8Array(0))"));
                    }
                    if matches!(&field.ty, Ty::List(_)) {
                        return self.encode_value(&field.ty, &format!("({source} ?? [])"));
                    }
                    if matches!(&field.ty, Ty::Map(_, _)) {
                        return self.encode_value(&field.ty, &format!("({source} ?? {{}})"));
                    }
                    Ok(format!("c.present({source}, (value) => {convert})"))
                } else {
                    self.encode_value(&field.ty, source)
                }
            }
        }
    }

    fn decode_field(
        &mut self,
        field: &Field,
        source: &str,
        skip: Option<&str>,
    ) -> ToolResult<String> {
        if json_body(&field.ty) {
            return Ok(format!(
                "{source} === undefined ? undefined : c.decodeJson({source})"
            ));
        }
        match &field.ty {
            Ty::Option(inner) => match &**inner {
                Ty::Option(value) => {
                    let convert = self.decode_element(value, "change.value")?;
                    Ok(format!(
                        "c.decodePatch({source}?.change, (change) => {convert})"
                    ))
                }
                _ => {
                    let convert = self.decode_element(inner, "value")?;
                    let absent = if skip == Some("Option::is_none") {
                        "undefined"
                    } else {
                        "null"
                    };
                    Ok(format!(
                        "{source} === undefined ? {absent} : ((value) => {convert})({source})"
                    ))
                }
            },
            Ty::List(_) | Ty::Map(_, _) => self.decode_value(&field.ty, source),
            _ => self.decode_value(
                &field.ty,
                &format!("c.required({source}, {:?})", field.name),
            ),
        }
    }

    fn encode_element(&mut self, ty: &Ty, source: &str) -> ToolResult<String> {
        if matches!(ty, Ty::List(_) | Ty::Map(_, _) | Ty::Option(_)) {
            self.ensure(ty)?;
            Ok(format!("encode{}({source})", self.model.proto_name(ty)))
        } else {
            self.encode_value(ty, source)
        }
    }
    fn decode_element(&mut self, ty: &Ty, source: &str) -> ToolResult<String> {
        if matches!(ty, Ty::List(_) | Ty::Map(_, _) | Ty::Option(_)) {
            self.ensure(ty)?;
            Ok(format!("decode{}({source})", self.model.proto_name(ty)))
        } else {
            self.decode_value(ty, source)
        }
    }
    fn encode_value(&mut self, ty: &Ty, source: &str) -> ToolResult<String> {
        Ok(match ty {
            Ty::Scalar(s) => match s.as_str() {
                "String" => format!("c.text({source})"),
                "bool" => format!("c.bool({source})"),
                "i64" => format!("c.int64({source})"),
                "u64" | "usize" => format!("c.int64({source}, true)"),
                "i32" => format!("c.int32({source})"),
                "u32" => format!("c.int32({source}, true)"),
                "u8" => format!("c.int32({source}, true, 255)"),
                "f32" => format!("c.float({source}, true)"),
                "f64" => format!("c.float({source})"),
                _ => unreachable!(),
            },
            Ty::Json => format!("c.encodeJson({source})"),
            Ty::Named(_, _) => {
                self.ensure(ty)?;
                format!("encode{}({source})", self.model.proto_name(ty))
            }
            Ty::List(inner) if **inner == Ty::Scalar("u8".into()) => format!("c.bytes({source})"),
            Ty::List(inner) => {
                let convert = self.encode_element(inner, "value")?;
                format!("c.array({source}, (value) => {convert})")
            }
            Ty::Map(_, inner) => {
                let convert = self.encode_element(inner, "value")?;
                format!("c.dictionary({source}, (value) => {convert})")
            }
            Ty::Unit => "create(EmptySchema)".into(),
            Ty::Option(_) => unreachable!(),
        })
    }
    fn decode_value(&mut self, ty: &Ty, source: &str) -> ToolResult<String> {
        Ok(match ty {
            Ty::Scalar(s) if matches!(s.as_str(), "i64" | "u64" | "usize") => {
                format!("c.safeNumber({source})")
            }
            Ty::Scalar(s) if matches!(s.as_str(), "f32" | "f64") => format!("c.float({source})"),
            Ty::Scalar(_) => source.into(),
            Ty::Json => format!("c.decodeJson({source})"),
            Ty::Named(_, _) => {
                self.ensure(ty)?;
                format!("decode{}({source})", self.model.proto_name(ty))
            }
            Ty::List(inner) if **inner == Ty::Scalar("u8".into()) => source.into(),
            Ty::List(inner) => {
                let convert = self.decode_element(inner, "value")?;
                format!("{source}.map((value) => {convert})")
            }
            Ty::Map(_, inner) => {
                let convert = self.decode_element(inner, "value")?;
                format!(
                    "Object.fromEntries(Object.entries({source}).map(([key, value]) => [key, {convert}]))"
                )
            }
            Ty::Unit => "null".into(),
            Ty::Option(_) => unreachable!(),
        })
    }
}

fn fixtures(root: &Path, operations: &[Operation], check: bool) -> ToolResult<()> {
    use kanban_protocol::HttpTransportLocation as Location;
    let catalog = kanban_protocol::operation_catalog();
    let mut tests = String::from(
        "// @generated: xtask rpc-codegen；既有业务 fixture 的 Protobuf 往返。\nimport { test, expect } from \"vitest\"\nimport { create, toBinary, fromBinary } from \"@bufbuild/protobuf\"\nimport * as s from \"../../generated/rpc/kanban/v1/kanban_pb\"\nimport * as d from \"../../generated/rpc/kanban/v1/dto_pb\"\nimport * as c from \"./codec.generated\"\n",
    );
    for operation in operations {
        let Some(parent) = catalog
            .iter()
            .find(|parent| parent.operation_id == operation.id)
        else {
            continue;
        };
        let method = &operation.method;
        let mut request = String::new();
        for (part, location) in [
            ("path", Location::Path),
            ("query", Location::Query),
            ("input", Location::Body),
        ] {
            if let Some(fixture) = parent
                .contracts
                .iter()
                .find(|binding| binding.location == Some(location))
                .and_then(|binding| binding.valid_fixture)
            {
                let fixture: serde_json::Value =
                    serde_json::from_slice(&fs::read(root.join(fixture))?)?;
                let value = serde_json::to_string(&fixture)?;
                if method == "CreateAttachment" && part == "input" {
                    request.push_str(&format!(
                        ", input: {{ ...{value}, content: new Uint8Array({value}.content ?? []) }}"
                    ));
                } else {
                    request.push_str(&format!(", {part}: {value}"));
                }
            }
        }
        tests.push_str(&format!("test({:?}, () => {{ const message = c.encode{method}Request({{ method: {method:?}{request} }}); expect(fromBinary(s.{method}RequestSchema, toBinary(s.{method}RequestSchema, message))).toEqual(message) }})\n", format!("{method} request fixture")));
        if matches!(
            method.as_str(),
            "DownloadAttachment" | "QueryLabelAtomIndex"
        ) {
            continue;
        }
        if let Some(fixture) = parent
            .contracts
            .iter()
            .find(|binding| binding.location == Some(Location::Success))
            .and_then(|binding| binding.valid_fixture)
        {
            let fixture: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join(fixture))?)?;
            let value = serde_json::to_string(&fixture)?;
            tests.push_str(&format!("test({:?}, () => {{ const fixture = {value}; const message = c.encode{method}Response(fixture); const decoded = c.decode{method}Response(fromBinary(d.{method}ResponseSchema, toBinary(d.{method}ResponseSchema, message))); expect(decoded).toMatchObject(fixture); expect(c.encode{method}Response(decoded)).toEqual(create(d.{method}ResponseSchema, message)) }})\n", format!("{method} response fixture")));
        }
    }
    write(
        &root.join("apps/web/src/lib/rpc/fixtures.generated.test.ts"),
        &tests,
        check,
    )
}
