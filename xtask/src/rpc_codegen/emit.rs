//! 从已解析且消歧的类型生成正式 proto 与字段级 Rust codec。

use std::collections::{BTreeMap, BTreeSet};

use super::model::{Field, Model, Ty, snake};
use crate::ToolResult;

pub struct Emitter<'a> {
    pub model: &'a Model,
    pub messages: BTreeMap<String, String>,
    pub codecs: BTreeMap<String, String>,
    visiting: BTreeSet<String>,
}

impl<'a> Emitter<'a> {
    pub fn new(model: &'a Model) -> Self {
        Self {
            model,
            messages: BTreeMap::new(),
            codecs: BTreeMap::new(),
            visiting: BTreeSet::new(),
        }
    }

    pub fn ensure(&mut self, ty: &Ty) -> ToolResult<String> {
        if let Some(p) = scalar(ty) {
            return Ok(p.into());
        }
        if matches!(ty, Ty::Json) || is_json_body(ty) {
            return Ok("JsonValue".into());
        }
        if matches!(ty, Ty::Unit) {
            return Ok("Empty".into());
        }
        let name = self.model.proto_name(ty);
        if self.visiting.contains(&name) {
            return Ok(name);
        }
        self.visiting.insert(name.clone());
        match ty {
            Ty::Named(_, _) if self.model.is_enum(ty) => self.emit_enum(ty, &name)?,
            Ty::Named(_, _) if self.model.is_tuple(ty) => self.emit_tuple(ty, &name)?,
            Ty::Named(_, _) => {
                let fields = self.model.fields(ty)?;
                self.emit_struct(&name, ty, &fields)?;
            }
            Ty::List(inner) => {
                let field = Field {
                    name: "items".into(),
                    ty: ty.clone(),
                    default: None,
                };
                let line = self.field_line(&field, 1)?;
                self.messages
                    .insert(name.clone(), format!("message {name} {{\n{line}\n}}\n"));
                self.ensure(inner)?;
            }
            Ty::Map(_, inner) => {
                let field = Field {
                    name: "entries".into(),
                    ty: ty.clone(),
                    default: None,
                };
                let line = self.field_line(&field, 1)?;
                self.messages
                    .insert(name.clone(), format!("message {name} {{\n{line}\n}}\n"));
                self.ensure(inner)?;
            }
            Ty::Option(inner) => {
                // 作为集合元素时，optional 也必须有独立 message。
                let field = Field {
                    name: "value".into(),
                    ty: ty.clone(),
                    default: None,
                };
                let line = self.field_line(&field, 1)?;
                self.messages
                    .insert(name.clone(), format!("message {name} {{\n{line}\n}}\n"));
                self.ensure(inner)?;
            }
            _ => return Err(format!("不能生成 {ty:?}").into()),
        }
        Ok(name)
    }

    fn patch(&mut self, inner: &Ty) -> ToolResult<String> {
        let name = format!("Patch{}", self.model.proto_name(inner));
        if !self.messages.contains_key(&name) {
            let p = self.ensure(inner)?;
            self.messages.insert(name.clone(),format!("// 外层缺失表示不修改；clear/value 是明确的写入意图。\nmessage {name} {{\n  oneof change {{\n    Empty clear = 1;\n    {p} value = 2;\n  }}\n}}\n"));
        }
        Ok(name)
    }

    pub fn field_line(&mut self, field: &Field, index: usize) -> ToolResult<String> {
        let name = &field.name;
        let ty = &field.ty;
        let (prefix, p) = match ty {
            Ty::Option(inner) => match &**inner {
                Ty::Option(value) => ("", self.patch(value)?),
                _ => {
                    let p = self.ensure(inner)?;
                    (
                        if self.scalar_like(inner) {
                            "optional "
                        } else {
                            ""
                        },
                        p,
                    )
                }
            },
            Ty::List(inner) if **inner == Ty::Scalar("u8".into()) => ("", "bytes".into()),
            Ty::List(inner) => ("repeated ", self.ensure(inner)?),
            Ty::Map(_, inner) => ("", format!("map<string, {}>", self.ensure(inner)?)),
            _ => {
                let p = self.ensure(ty)?;
                (
                    if self.scalar_like(ty) {
                        "optional "
                    } else {
                        ""
                    },
                    p,
                )
            }
        };
        Ok(format!("  {prefix}{p} {name} = {index};"))
    }

    fn scalar_like(&self, ty: &Ty) -> bool {
        scalar(ty).is_some()
            || (self.model.is_enum(ty)
                && self
                    .model
                    .variants(ty)
                    .is_ok_and(|vs| vs.iter().all(|(_, f, _)| f.is_empty())))
    }

    pub fn encode_field(&mut self, field: &Field, expr: &str) -> ToolResult<String> {
        let ty = &field.ty;
        if is_json_body(ty) {
            return Ok(format!(
                "match {expr} {{ crate::JsonBodyFieldWire::Missing => None, crate::JsonBodyFieldWire::Present(value) => Some(encode_json(value)?), }}"
            ));
        }
        match ty {
            Ty::Option(inner) => match &**inner {
                Ty::Option(value) => {
                    let p = self.patch(value)?;
                    let variant = format!("v1::{}::Change", snake(&p));
                    let convert = self.encode_element(value, "value")?;
                    Ok(format!(
                        "{expr}.map(|change| -> Result<_, RpcCodecError> {{ Ok(v1::{p} {{ change: Some(match change {{ None => {variant}::Clear(v1::Empty{{}}), Some(value) => {variant}::Value({convert}), }}) }}) }}).transpose()?"
                    ))
                }
                _ => {
                    let convert = self.encode_element(inner, "value")?;
                    if convert == "value" {
                        return Ok(expr.into());
                    }
                    let result = as_result(&convert);
                    Ok(format!(
                        "{expr}.map(|value| -> Result<_, RpcCodecError> {{ {result} }}).transpose()?"
                    ))
                }
            },
            Ty::List(_) | Ty::Map(_, _) => self.encode_value(ty, expr),
            _ => Ok(format!("Some({})", self.encode_value(ty, expr)?)),
        }
    }

    pub fn decode_field(&mut self, field: &Field, expr: &str) -> ToolResult<String> {
        let ty = &field.ty;
        if is_json_body(ty) {
            return Ok(format!(
                "match {expr} {{ None => crate::JsonBodyFieldWire::Missing, Some(value) => crate::JsonBodyFieldWire::Present(decode_json(value)?), }}"
            ));
        }
        match ty {
            Ty::Option(inner) => match &**inner {
                Ty::Option(value) => {
                    let p = self.patch(value)?;
                    let variant = format!("v1::{}::Change", snake(&p));
                    let convert = self.decode_element(value, "value")?;
                    Ok(format!(
                        "{expr}.map(|patch| -> Result<_, RpcCodecError> {{ Ok(match required(patch.change, {:?})? {{ {variant}::Clear(_) => None, {variant}::Value(value) => Some({convert}), }}) }}).transpose()?",
                        field.name
                    ))
                }
                _ => {
                    let convert = self.decode_element(inner, "value")?;
                    if convert == "value" {
                        return Ok(expr.into());
                    }
                    let result = as_result(&convert);
                    Ok(format!(
                        "{expr}.map(|value| -> Result<_, RpcCodecError> {{ {result} }}).transpose()?"
                    ))
                }
            },
            Ty::List(_) | Ty::Map(_, _) => self.decode_value(ty, expr),
            _ => {
                if let Some(default) = &field.default {
                    let convert = self.decode_value(ty, "value")?;
                    if convert == "value" {
                        let literal = default
                            .trim()
                            .strip_prefix('{')
                            .and_then(|value| value.strip_suffix('}'))
                            .map(str::trim);
                        if let Some(literal) = literal.filter(|value| {
                            matches!(syn::parse_str::<syn::Expr>(value), Ok(syn::Expr::Lit(_)))
                        }) {
                            return Ok(format!("{expr}.unwrap_or({literal})"));
                        }
                        return Ok(if default == "Default::default()" {
                            format!("{expr}.unwrap_or_default()")
                        } else {
                            format!("{expr}.unwrap_or_else(|| {default})")
                        });
                    }
                    Ok(format!(
                        "match {expr} {{ Some(value) => {convert}, None => {default}, }}"
                    ))
                } else {
                    self.decode_value(ty, &format!("required({expr}, {:?})?", field.name))
                }
            }
        }
    }

    fn encode_value(&mut self, ty: &Ty, expr: &str) -> ToolResult<String> {
        Ok(match ty {
            Ty::Scalar(s) if s == "usize" => format!(
                "u64::try_from({expr}).map_err(|_| RpcCodecError::invalid(\"usize 超出 uint64\"))?"
            ),
            Ty::Scalar(s) if s == "u8" => format!("u32::from({expr})"),
            Ty::Scalar(s) if s == "f32" || s == "f64" => format!("finite({expr})?"),
            Ty::Scalar(_) => expr.into(),
            Ty::Json => format!("encode_json({expr})?"),
            Ty::Named(_, _) if self.scalar_like(ty) => {
                let p = self.ensure(ty)?;
                format!("i32::from(v1::{p}::try_from({expr})?)")
            }
            Ty::Named(_, _) => format!("({expr}).try_into()?"),
            Ty::List(inner) if **inner == Ty::Scalar("u8".into()) => expr.into(),
            Ty::List(inner) => {
                let conv = self.encode_element(inner, "value")?;
                if conv == "value" {
                    return Ok(expr.into());
                }
                let result = as_result(&conv);
                format!(
                    "({expr}).into_iter().map(|value| -> Result<_, RpcCodecError> {{ {result} }}).collect::<Result<Vec<_>, _>>()?"
                )
            }
            Ty::Map(_, inner) => {
                let conv = self.encode_element(inner, "value")?;
                format!(
                    "({expr}).into_iter().map(|(key,value)| -> Result<_, RpcCodecError> {{ Ok((key,{conv})) }}).collect::<Result<_, _>>()?"
                )
            }
            Ty::Option(_) => return Err("encode_value 的 Option 需要 wrapper".into()),
            Ty::Unit => "v1::Empty{}".into(),
        })
    }

    fn decode_value(&mut self, ty: &Ty, expr: &str) -> ToolResult<String> {
        Ok(match ty {
            Ty::Scalar(s) if s == "usize" => format!(
                "usize::try_from({expr}).map_err(|_| RpcCodecError::invalid(\"uint64 超出 usize\"))?"
            ),
            Ty::Scalar(s) if s == "u8" => format!(
                "u8::try_from({expr}).map_err(|_| RpcCodecError::invalid(\"uint32 超出 u8\"))?"
            ),
            Ty::Scalar(s) if s == "f32" || s == "f64" => format!("finite({expr})?"),
            Ty::Scalar(_) => expr.into(),
            Ty::Json => format!("decode_json({expr})?"),
            Ty::Named(_, _) if self.scalar_like(ty) => {
                let p = self.ensure(ty)?;
                format!(
                    "v1::{p}::try_from({expr}).map_err(|_| RpcCodecError::invalid(\"未知 enum 数值\"))?.try_into()?"
                )
            }
            Ty::Named(_, _) => format!("({expr}).try_into()?"),
            Ty::List(inner) if **inner == Ty::Scalar("u8".into()) => expr.into(),
            Ty::List(inner) => {
                let conv = self.decode_element(inner, "value")?;
                if conv == "value" {
                    return Ok(expr.into());
                }
                let result = as_result(&conv);
                format!(
                    "({expr}).into_iter().map(|value| -> Result<_, RpcCodecError> {{ {result} }}).collect::<Result<Vec<_>, _>>()?"
                )
            }
            Ty::Map(_, inner) => {
                let conv = self.decode_element(inner, "value")?;
                format!(
                    "({expr}).into_iter().map(|(key,value)| -> Result<_, RpcCodecError> {{ Ok((key,{conv})) }}).collect::<Result<_, _>>()?"
                )
            }
            Ty::Option(_) => return Err("decode_value 的 Option 需要 wrapper".into()),
            Ty::Unit => "()".into(),
        })
    }

    fn encode_element(&mut self, ty: &Ty, expr: &str) -> ToolResult<String> {
        match ty {
            Ty::List(_) | Ty::Map(_, _) | Ty::Option(_) => {
                let p = self.ensure(ty)?;
                let name = if matches!(ty, Ty::List(_)) {
                    "items"
                } else if matches!(ty, Ty::Map(_, _)) {
                    "entries"
                } else {
                    "value"
                };
                let field = Field {
                    name: name.into(),
                    ty: ty.clone(),
                    default: None,
                };
                Ok(format!(
                    "v1::{p}{{ {name}:{} }}",
                    self.encode_field(&field, expr)?
                ))
            }
            _ => self.encode_value(ty, expr),
        }
    }

    fn decode_element(&mut self, ty: &Ty, expr: &str) -> ToolResult<String> {
        match ty {
            Ty::List(_) | Ty::Map(_, _) | Ty::Option(_) => {
                let name = if matches!(ty, Ty::List(_)) {
                    "items"
                } else if matches!(ty, Ty::Map(_, _)) {
                    "entries"
                } else {
                    "value"
                };
                let field = Field {
                    name: name.into(),
                    ty: ty.clone(),
                    default: None,
                };
                self.decode_field(&field, &format!("({expr}).{name}"))
            }
            _ => self.decode_value(ty, expr),
        }
    }

    pub fn emit_struct(&mut self, name: &str, ty: &Ty, fields: &[Field]) -> ToolResult<()> {
        let body = fields
            .iter()
            .enumerate()
            .map(|(i, f)| self.field_line(f, i + 1))
            .collect::<ToolResult<Vec<_>>>()?
            .join("\n");
        self.messages.insert(
            name.into(),
            format!("// {}\nmessage {name} {{\n{body}\n}}\n", ty.rust()),
        );
        let encode = fields
            .iter()
            .map(|f| {
                Ok(format!(
                    "{}: {},",
                    ident(&f.name),
                    self.encode_field(f, &format!("dto.{}", ident(&f.name)))?
                ))
            })
            .collect::<ToolResult<String>>()?;
        let decode = fields
            .iter()
            .map(|f| {
                Ok(format!(
                    "{}: {},",
                    ident(&f.name),
                    self.decode_field(f, &format!("wire.{}", ident(&f.name)))?
                ))
            })
            .collect::<ToolResult<String>>()?;
        let Ty::Named(base, _) = ty else {
            return Err("struct 需要named类型".into());
        };
        let rust = ty.rust();
        let extra = if base == "crate::event_stream::StreamEventData" {
            "validate_stream_event(&result)?;"
        } else {
            ""
        };
        let unused = if fields.is_empty() {
            ("let _=dto;", "let _=wire;")
        } else {
            ("", "")
        };
        let code = format!(
            "impl TryFrom<{rust}> for v1::{name} {{ type Error = RpcCodecError; fn try_from(dto: {rust}) -> Result<Self, Self::Error> {{ {} Ok(Self {{ {encode} }}) }} }}\nimpl TryFrom<v1::{name}> for {rust} {{ type Error = RpcCodecError; fn try_from(wire: v1::{name}) -> Result<Self, Self::Error> {{ {} let result: Self = {base} {{ {decode} }}; {extra} Ok(result) }} }}\n",
            unused.0, unused.1
        );
        self.codecs.insert(name.into(), code);
        Ok(())
    }

    fn emit_tuple(&mut self, ty: &Ty, name: &str) -> ToolResult<()> {
        let mut fields = self.model.fields(ty)?;
        if fields.len() != 1 {
            return Err(format!("不支持多字段 tuple: {}", ty.rust()).into());
        }
        fields[0].name = "value".into();
        let f = &fields[0];
        let body = self.field_line(f, 1)?;
        self.messages
            .insert(name.into(), format!("message {name} {{\n{body}\n}}\n"));
        let rust = ty.rust();
        let last = rust.rsplit("::").next().unwrap();
        let get = match last {
            "ApiTaskPriority" | "UnitInterval" | "PositiveRank" => "dto.get()",
            "TaskReadLabel" => "dto.into_string()",
            _ => "dto.0",
        };
        let encode = self.encode_field(f, get)?;
        let decoded = self.decode_field(f, "wire.value")?;
        let ctor = match last {
            "ApiTaskPriority" | "TaskReadLabel" => {
                format!("{rust}::new({decoded}).ok_or_else(|| RpcCodecError::invalid({last:?}))?")
            }
            "UnitInterval" | "PositiveRank" => {
                format!("{rust}::new({decoded}).map_err(RpcCodecError::invalid)?")
            }
            _ => format!("{rust}({decoded})"),
        };
        let result = as_result(&ctor);
        self.codecs.insert(name.into(),format!("impl TryFrom<{rust}> for v1::{name} {{ type Error=RpcCodecError; fn try_from(dto:{rust})->Result<Self,Self::Error>{{Ok(Self{{value:{encode}}})}} }}\nimpl TryFrom<v1::{name}> for {rust} {{type Error=RpcCodecError; fn try_from(wire:v1::{name})->Result<Self,Self::Error>{{{result}}}}}\n"));
        Ok(())
    }

    fn emit_enum(&mut self, ty: &Ty, name: &str) -> ToolResult<()> {
        let variants = self.model.variants(ty)?;
        let rust = ty.rust();
        if variants.iter().all(|(_, f, _)| f.is_empty()) {
            let prefix = snake(name).to_uppercase();
            let body = variants
                .iter()
                .enumerate()
                .map(|(i, (n, _, _))| {
                    format!("  {prefix}_{} = {};", snake(n).to_uppercase(), i + 1)
                })
                .collect::<Vec<_>>()
                .join("\n");
            self.messages.insert(
                name.into(),
                format!("enum {name} {{\n  {prefix}_UNSPECIFIED = 0;\n{body}\n}}\n"),
            );
            let to = variants
                .iter()
                .map(|(n, _, _)| format!("{rust}::{n} => Self::{n},"))
                .collect::<String>();
            let from = variants
                .iter()
                .map(|(n, _, _)| format!("v1::{name}::{n} => Self::{n},"))
                .collect::<String>();
            self.codecs.insert(name.into(),format!("impl TryFrom<{rust}> for v1::{name} {{type Error=RpcCodecError;fn try_from(dto:{rust})->Result<Self,Self::Error>{{Ok(match dto {{{to}}})}}}}\nimpl TryFrom<v1::{name}> for {rust} {{type Error=RpcCodecError;fn try_from(wire:v1::{name})->Result<Self,Self::Error>{{Ok(match wire {{{from}v1::{name}::Unspecified=>return Err(RpcCodecError::invalid(\"enum 未指定\")),}})}}}}\n"));
            return Ok(());
        }
        let mut lines = Vec::new();
        let mut encode = String::new();
        let mut decode = String::new();
        let choice = format!("v1::{}::Value", snake(name));
        for (i, (variant, fields, named)) in variants.iter().enumerate() {
            let (wire_type, to, from) = if fields.len() == 1 && !named {
                let ft = &fields[0].ty;
                (
                    self.ensure(ft)?,
                    self.encode_element(ft, "value")?,
                    self.decode_element(ft, "value")?,
                )
            } else {
                let vn = format!("{name}{variant}");
                let body = fields
                    .iter()
                    .enumerate()
                    .map(|(i, f)| self.field_line(f, i + 1))
                    .collect::<ToolResult<Vec<_>>>()?
                    .join("\n");
                self.messages
                    .insert(vn.clone(), format!("message {vn} {{\n{body}\n}}\n"));
                let to = fields
                    .iter()
                    .map(|f| {
                        Ok(format!(
                            "{}:{},",
                            ident(&f.name),
                            self.encode_field(f, &ident(&f.name))?
                        ))
                    })
                    .collect::<ToolResult<String>>()?;
                let from = fields
                    .iter()
                    .map(|f| {
                        Ok(format!(
                            "{}:{},",
                            ident(&f.name),
                            self.decode_field(f, &format!("value.{}", ident(&f.name)))?
                        ))
                    })
                    .collect::<ToolResult<String>>()?;
                (vn.clone(), format!("v1::{vn}{{{to}}}"), from)
            };
            lines.push(format!("    {wire_type} {} = {};", snake(variant), i + 1));
            if fields.len() == 1 && !named {
                encode.push_str(&format!(
                    "{rust}::{variant}(value)=>{choice}::{variant}({to}),"
                ));
                decode.push_str(&format!(
                    "{choice}::{variant}(value)=>Self::{variant}({from}),"
                ));
            } else if *named {
                let args = fields
                    .iter()
                    .map(|f| ident(&f.name))
                    .collect::<Vec<_>>()
                    .join(",");
                encode.push_str(&format!(
                    "{rust}::{variant}{{{args}}}=>{choice}::{variant}({to}),"
                ));
                decode.push_str(&format!(
                    "{choice}::{variant}(value)=>Self::{variant}{{{from}}},"
                ));
            } else if fields.is_empty() {
                encode.push_str(&format!("{rust}::{variant}=>{choice}::{variant}({to}),"));
                decode.push_str(&format!("{choice}::{variant}(_)=>Self::{variant},"));
            } else {
                return Err("多位置 enum variant 暂不支持".into());
            }
        }
        self.messages.insert(
            name.into(),
            format!(
                "message {name} {{\n  oneof value {{\n{}\n  }}\n}}\n",
                lines.join("\n")
            ),
        );
        self.codecs.insert(name.into(),format!("impl TryFrom<{rust}> for v1::{name} {{type Error=RpcCodecError;fn try_from(dto:{rust})->Result<Self,Self::Error>{{Ok(Self{{value:Some(match dto{{{encode}}})}})}}}}\nimpl TryFrom<v1::{name}> for {rust} {{type Error=RpcCodecError;fn try_from(wire:v1::{name})->Result<Self,Self::Error>{{Ok(match required(wire.value,{name:?})?{{{decode}}})}}}}\n"));
        Ok(())
    }
}

fn scalar(ty: &Ty) -> Option<&'static str> {
    let Ty::Scalar(s) = ty else {
        return None;
    };
    Some(match s.as_str() {
        "String" => "string",
        "bool" => "bool",
        "i64" => "int64",
        "u64" | "usize" => "uint64",
        "i32" => "int32",
        "u32" | "u8" => "uint32",
        "f32" => "float",
        "f64" => "double",
        _ => return None,
    })
}

pub fn ident(s: &str) -> String {
    if matches!(
        s,
        "type"
            | "ref"
            | "match"
            | "loop"
            | "in"
            | "enum"
            | "struct"
            | "move"
            | "self"
            | "super"
            | "crate"
            | "where"
            | "async"
            | "await"
            | "use"
            | "mod"
    ) {
        format!("r#{s}")
    } else {
        s.into()
    }
}

fn is_json_body(ty: &Ty) -> bool {
    matches!(ty,Ty::Named(n,_) if n=="crate::label_surfaces::JsonBodyFieldWire")
}

fn as_result(expression: &str) -> String {
    expression
        .strip_suffix('?')
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Ok({expression})"))
}
