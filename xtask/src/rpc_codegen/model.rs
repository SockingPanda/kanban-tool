//! 解析真实 Rust item、use 与有限的 DTO 声明宏，按模块解析类型别名。

use std::{collections::BTreeMap, fs, path::Path};

use quote::ToTokens;
use syn::{Attribute, Fields, GenericArgument, Item, PathArguments, Type, UseTree};

use crate::ToolResult;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ty {
    Scalar(String),
    Json,
    Option(Box<Ty>),
    List(Box<Ty>),
    Map(String, Box<Ty>),
    Named(String, Vec<Ty>),
    Unit,
}

impl Ty {
    pub fn rust(&self) -> String {
        match self {
            Self::Scalar(s) => s.clone(),
            Self::Json => "serde_json::Value".into(),
            Self::Option(t) => format!("Option<{}>", t.rust()),
            Self::List(t) => format!("Vec<{}>", t.rust()),
            Self::Map(owner, t) => format!("{owner}<String,{}>", t.rust()),
            Self::Named(n, args) if args.is_empty() => n.clone(),
            Self::Named(n, args) => format!(
                "{n}<{}>",
                args.iter().map(Self::rust).collect::<Vec<_>>().join(",")
            ),
            Self::Unit => "()".into(),
        }
    }
}

#[derive(Clone)]
pub struct Definition {
    pub module: String,
    pub item: Item,
}

#[derive(Clone)]
pub struct Field {
    pub name: String,
    pub ty: Ty,
    pub default: Option<String>,
}

#[derive(Default)]
pub struct Model {
    pub defs: BTreeMap<String, Definition>,
    imports: BTreeMap<(String, String), String>,
    globs: BTreeMap<String, Vec<String>>,
    defaults: BTreeMap<(String, String), String>,
    constants: BTreeMap<(String, String), String>,
    simple_macros: BTreeMap<(String, String), String>,
}

pub fn pascal(s: &str) -> String {
    s.split(['_', ':', '-'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut cs = p.chars();
            cs.next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default()
                + cs.as_str()
        })
        .collect()
}

pub fn snake(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, c) in cs.iter().copied().enumerate() {
        if c.is_uppercase()
            && i > 0
            && (cs[i - 1].is_lowercase()
                || cs[i - 1].is_ascii_digit()
                || cs.get(i + 1).is_some_and(|c| c.is_lowercase()))
        {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

impl Model {
    pub fn read(root: &Path) -> ToolResult<Self> {
        let mut model = Self::default();
        model.read_module(&root.join("src/lib.rs"), "crate")?;
        // 迁移专用 wire DTO 尚不要求修改原 HTTP/schema inventory。
        model.read_module(&root.join("src/rpc/dto.rs"), "crate::rpc::dto")?;
        Ok(model)
    }

    fn read_module(&mut self, path: &Path, module: &str) -> ToolResult<()> {
        let file = syn::parse_file(&fs::read_to_string(path)?)?;
        for item in file.items {
            self.add_item(item, module, path)?;
        }
        Ok(())
    }

    fn add_item(&mut self, item: Item, module: &str, path: &Path) -> ToolResult<()> {
        match &item {
            Item::Use(u) => self.add_use(&u.tree, module, String::new()),
            Item::Struct(s) => {
                self.defs.insert(
                    format!("{module}::{}", s.ident),
                    Definition {
                        module: module.into(),
                        item,
                    },
                );
            }
            Item::Enum(s) => {
                self.defs.insert(
                    format!("{module}::{}", s.ident),
                    Definition {
                        module: module.into(),
                        item,
                    },
                );
            }
            Item::Type(s) => {
                self.defs.insert(
                    format!("{module}::{}", s.ident),
                    Definition {
                        module: module.into(),
                        item,
                    },
                );
            }
            Item::Fn(f) => {
                if f.sig.ident.to_string().starts_with("default_") {
                    self.defaults.insert(
                        (module.into(), f.sig.ident.to_string()),
                        f.block.to_token_stream().to_string(),
                    );
                }
            }
            Item::Const(c) => {
                self.constants.insert(
                    (module.into(), c.ident.to_string()),
                    c.expr.to_token_stream().to_string(),
                );
            }
            Item::Mod(m)
                if !matches!(
                    m.ident.to_string().as_str(),
                    "rpc" | "tests" | "protocol_tests" | "lifecycle_tests" | "schema"
                ) =>
            {
                if let Some((_, items)) = &m.content {
                    for nested in items {
                        self.add_item(nested.clone(), &format!("{module}::{}", m.ident), path)?;
                    }
                } else {
                    let dir = if module == "crate" {
                        path.parent().unwrap().to_path_buf()
                    } else {
                        path.with_extension("")
                    };
                    let next = dir.join(format!("{}.rs", m.ident));
                    if next.exists() {
                        self.read_module(&next, &format!("{module}::{}", m.ident))?;
                    }
                }
            }
            Item::Macro(m) => {
                let name = m.mac.path.segments.last().unwrap().ident.to_string();
                if name == "macro_rules" {
                    let tokens = m.mac.tokens.to_string();
                    if let Some((pattern, body)) = tokens.split_once("=>")
                        && (pattern.trim() == "($ name : ident)"
                            || pattern.replace(' ', "") == "($name:ident)")
                    {
                        let body = body.trim().trim_end_matches(';').trim();
                        if let Some(inner) =
                            body.strip_prefix('{').and_then(|b| b.strip_suffix('}'))
                        {
                            self.simple_macros.insert(
                                (module.into(), m.ident.as_ref().unwrap().to_string()),
                                inner.into(),
                            );
                        }
                    }
                }
                match name.as_str() {
                    "wire" | "cli_wire" => {
                        let mut item: Item = syn::parse2(m.mac.tokens.clone())?;
                        if let Item::Enum(item) = &mut item {
                            item.attrs
                                .push(syn::parse_quote!(#[serde(rename_all = "snake_case")]));
                        }
                        self.add_item(item, module, path)?
                    }
                    "payload" => {
                        let tokens = m.mac.tokens.to_string();
                        let parsed: syn::ItemStruct =
                            syn::parse_str(&format!("pub struct {tokens}"))?;
                        self.add_item(Item::Struct(parsed), module, path)?;
                    }
                    "string_wire_enum" => {
                        let parsed = syn::parse2::<StringWireEnum>(m.mac.tokens.clone())?;
                        self.add_item(Item::Enum(parsed.0), module, path)?;
                    }
                    _ => {
                        if let Some(template) =
                            self.simple_macros.get(&(module.into(), name)).cloned()
                        {
                            let argument =
                                syn::parse2::<syn::Ident>(m.mac.tokens.clone())?.to_string();
                            let expanded = template.replace("$ name", &argument);
                            for item in syn::parse_file(&expanded)?.items {
                                self.add_item(item, module, path)?;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn add_use(&mut self, tree: &UseTree, module: &str, prefix: String) {
        match tree {
            UseTree::Path(p) => self.add_use(&p.tree, module, format!("{prefix}{}::", p.ident)),
            UseTree::Group(g) => {
                for t in &g.items {
                    self.add_use(t, module, prefix.clone());
                }
            }
            UseTree::Name(n) => {
                self.imports.insert(
                    (module.into(), n.ident.to_string()),
                    format!("{prefix}{}", n.ident),
                );
            }
            UseTree::Rename(n) => {
                self.imports.insert(
                    (module.into(), n.rename.to_string()),
                    format!("{prefix}{}", n.ident),
                );
            }
            UseTree::Glob(_) => self
                .globs
                .entry(module.into())
                .or_default()
                .push(prefix.trim_end_matches("::").into()),
        }
    }

    fn name(&self, raw: &str, module: &str, depth: usize) -> ToolResult<String> {
        if depth > 40 {
            return Err(format!("循环 use: {module} {raw}").into());
        }
        if raw.starts_with("serde_json::") || raw.starts_with("std::") {
            return Ok(raw.into());
        }
        if raw.starts_with("crate::") {
            if self.defs.contains_key(raw) {
                return Ok(raw.into());
            }
            let (owner, tail) = raw.rsplit_once("::").unwrap();
            return self.name(tail, owner, depth + 1);
        }
        if let Some(rest) = raw.strip_prefix("self::") {
            return self.name(rest, module, depth + 1);
        }
        if let Some(rest) = raw.strip_prefix("super::") {
            return self.name(
                rest,
                module.rsplit_once("::").map(|p| p.0).unwrap_or("crate"),
                depth + 1,
            );
        }
        let local = format!("{module}::{raw}");
        if self.defs.contains_key(&local) {
            return Ok(local);
        }
        let (head, tail) = raw
            .split_once("::")
            .map(|(a, b)| (a, format!("::{b}")))
            .unwrap_or((raw, String::new()));
        if let Some(import) = self.imports.get(&(module.into(), head.into())) {
            return self.name(&format!("{import}{tail}"), module, depth + 1);
        }
        for glob in self.globs.get(module).into_iter().flatten() {
            let target = if glob.starts_with("crate") {
                format!("{glob}::{raw}")
            } else {
                format!("{module}::{glob}::{raw}")
            };
            if self.defs.contains_key(&target) {
                return Ok(target);
            }
        }
        if raw.contains("::") && self.defs.contains_key(&format!("crate::{raw}")) {
            return Ok(format!("crate::{raw}"));
        }
        Err(format!("无法解析类型 {raw}，模块 {module}").into())
    }

    pub fn parse(&self, s: &str) -> ToolResult<Ty> {
        self.resolve(&syn::parse_str(s)?, "crate", &BTreeMap::new())
    }

    fn resolve(&self, ty: &Type, module: &str, vars: &BTreeMap<String, Ty>) -> ToolResult<Ty> {
        if matches!(ty,Type::Tuple(t) if t.elems.is_empty()) {
            return Ok(Ty::Unit);
        }
        let Type::Path(p) = ty else {
            return Err(format!("不支持的 wire 类型 {}", ty.to_token_stream()).into());
        };
        let raw = p
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        if let Some(t) = vars.get(&raw) {
            return Ok(t.clone());
        }
        let args = match &p.path.segments.last().unwrap().arguments {
            PathArguments::AngleBracketed(a) => a
                .args
                .iter()
                .filter_map(|a| {
                    if let GenericArgument::Type(t) = a {
                        Some(t)
                    } else {
                        None
                    }
                })
                .map(|t| self.resolve(t, module, vars))
                .collect::<ToolResult<Vec<_>>>()?,
            _ => Vec::new(),
        };
        match raw.as_str() {
            "String" | "bool" | "i64" | "u64" | "i32" | "u32" | "u8" | "usize" | "f32" | "f64" => {
                return Ok(Ty::Scalar(raw));
            }
            "Option" => return Ok(Ty::Option(Box::new(args[0].clone()))),
            "Vec" => return Ok(Ty::List(Box::new(args[0].clone()))),
            _ => {}
        }
        let name = self.name(&raw, module, 0)?;
        match name.as_str() {
            "serde_json::Value" => return Ok(Ty::Json),
            "std::collections::BTreeMap" | "std::collections::HashMap" | "serde_json::Map" => {
                if args[0] != Ty::Scalar("String".into()) {
                    return Err("proto map 仅允许 String key".into());
                }
                return Ok(Ty::Map(name, Box::new(args[1].clone())));
            }
            _ => {}
        }
        let def = self
            .defs
            .get(&name)
            .ok_or_else(|| format!("未知定义 {name}"))?;
        if let Item::Type(alias) = &def.item {
            let params = alias
                .generics
                .type_params()
                .map(|p| p.ident.to_string())
                .zip(args.iter().cloned())
                .collect();
            return self.resolve(&alias.ty, &def.module, &params);
        }
        Ok(Ty::Named(name, args))
    }

    pub fn fields(&self, ty: &Ty) -> ToolResult<Vec<Field>> {
        let Ty::Named(name, args) = ty else {
            return Err(format!("预期 named struct，得到 {ty:?}").into());
        };
        let def = &self.defs[name];
        let Item::Struct(s) = &def.item else {
            return Err(format!("预期 struct {name}").into());
        };
        let vars = s
            .generics
            .type_params()
            .map(|p| p.ident.to_string())
            .zip(args.iter().cloned())
            .collect();
        let defaults = serde_setting(&s.attrs, "default").is_some();
        let mut out = Vec::new();
        for (i, f) in s.fields.iter().enumerate() {
            let fname = f
                .ident
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| i.to_string());
            let ft = self.resolve(&f.ty, &def.module, &vars)?;
            let default = match serde_setting(&f.attrs, "default") {
                Some(Some(fun)) => {
                    let body = self
                        .defaults
                        .get(&(def.module.clone(), fun.clone()))
                        .ok_or_else(|| format!("缺少默认函数 {fun}"))?;
                    Some(
                        body.split_whitespace()
                            .map(|token| {
                                if let Some(value) =
                                    self.constants.get(&(def.module.clone(), token.into()))
                                {
                                    value.clone()
                                } else if self
                                    .defs
                                    .contains_key(&format!("{}::{token}", def.module))
                                {
                                    format!("{}::{token}", def.module)
                                } else {
                                    token.into()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                    )
                }
                Some(None) => Some("Default::default()".into()),
                None if defaults => Some(format!("<{}>::default().{fname}", ty.rust())),
                None => None,
            };
            out.push(Field {
                name: fname,
                ty: ft,
                default,
            });
        }
        Ok(out)
    }

    pub fn variants(&self, ty: &Ty) -> ToolResult<Vec<(String, Vec<Field>, bool)>> {
        let Ty::Named(name, args) = ty else {
            return Err("enum 预期 named".into());
        };
        let def = &self.defs[name];
        let Item::Enum(s) = &def.item else {
            return Err("预期 enum".into());
        };
        let vars = s
            .generics
            .type_params()
            .map(|p| p.ident.to_string())
            .zip(args.iter().cloned())
            .collect();
        s.variants
            .iter()
            .map(|v| {
                Ok((
                    v.ident.to_string(),
                    v.fields
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            Ok(Field {
                                name: f
                                    .ident
                                    .as_ref()
                                    .map(ToString::to_string)
                                    .unwrap_or_else(|| format!("value_{i}")),
                                ty: self.resolve(&f.ty, &def.module, &vars)?,
                                default: None,
                            })
                        })
                        .collect::<ToolResult<Vec<_>>>()?,
                    matches!(v.fields, Fields::Named(_)),
                ))
            })
            .collect()
    }

    pub fn is_enum(&self, ty: &Ty) -> bool {
        matches!(ty,Ty::Named(n,_) if matches!(self.defs[n].item,Item::Enum(_)))
    }

    pub fn json_field(&self, ty: &Ty, field: &str) -> (String, Option<String>) {
        let Ty::Named(name, _) = ty else {
            return (field.into(), None);
        };
        let Item::Struct(item) = &self.defs[name].item else {
            return (field.into(), None);
        };
        let source = item
            .fields
            .iter()
            .find(|f| f.ident.as_ref().is_some_and(|name| name == field));
        let rename = source.and_then(|f| serde_setting(&f.attrs, "rename").flatten());
        let name = rename.unwrap_or_else(|| {
            serde_case(
                field,
                serde_setting(&item.attrs, "rename_all")
                    .flatten()
                    .as_deref(),
            )
        });
        let skip = source.and_then(|f| serde_setting(&f.attrs, "skip_serializing_if").flatten());
        (name, skip)
    }

    pub fn json_variant(&self, ty: &Ty, variant: &str) -> String {
        let Ty::Named(name, _) = ty else {
            unreachable!()
        };
        let Item::Enum(item) = &self.defs[name].item else {
            unreachable!()
        };
        let variant = item.variants.iter().find(|v| v.ident == variant).unwrap();
        serde_setting(&variant.attrs, "rename")
            .flatten()
            .unwrap_or_else(|| {
                serde_case(
                    &variant.ident.to_string(),
                    serde_setting(&item.attrs, "rename_all")
                        .flatten()
                        .as_deref(),
                )
            })
    }
    pub fn is_tuple(&self, ty: &Ty) -> bool {
        matches!(ty,Ty::Named(n,_) if matches!(&self.defs[n].item,Item::Struct(s) if matches!(s.fields,Fields::Unnamed(_))))
    }
    pub fn proto_name(&self, ty: &Ty) -> String {
        match ty {
            Ty::Named(n, args) => {
                let short = n.rsplit("::").next().unwrap();
                let duplicates = self
                    .defs
                    .keys()
                    .filter(|p| p.ends_with(&format!("::{short}")))
                    .count()
                    > 1;
                let base = if duplicates {
                    format!("Dto{}", pascal(n.trim_start_matches("crate::")))
                } else {
                    format!("Dto{short}")
                };
                if args.is_empty() {
                    base
                } else {
                    format!(
                        "{base}Of{}",
                        args.iter()
                            .map(|t| self.proto_name(t))
                            .collect::<Vec<_>>()
                            .join("And")
                    )
                }
            }
            Ty::Scalar(s) => pascal(s),
            Ty::Json => "JsonValue".into(),
            Ty::Unit => "Empty".into(),
            Ty::List(t) => format!("ListOf{}", self.proto_name(t)),
            Ty::Option(t) => format!("Optional{}", self.proto_name(t)),
            Ty::Map(_, t) => format!("MapOf{}", self.proto_name(t)),
        }
    }
}

pub fn serde_setting(attrs: &[Attribute], key: &str) -> Option<Option<String>> {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let mut found = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(key) {
                if meta.input.peek(syn::Token![=]) {
                    found = Some(Some(meta.value()?.parse::<syn::LitStr>()?.value()));
                } else {
                    found = Some(None);
                }
            } else if meta.input.peek(syn::Token![=]) {
                let _: syn::Expr = meta.value()?.parse()?;
            }
            Ok(())
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

fn serde_case(name: &str, rule: Option<&str>) -> String {
    match rule {
        Some("snake_case") => snake(name),
        Some("camelCase") => {
            let upper = pascal(name);
            upper[..1].to_ascii_lowercase() + &upper[1..]
        }
        Some("lowercase") => name.to_lowercase(),
        Some("SCREAMING_SNAKE_CASE") => snake(name).to_uppercase(),
        Some("kebab-case") => snake(name).replace('_', "-"),
        _ => name.into(),
    }
}

struct StringWireEnum(syn::ItemEnum);

impl syn::parse::Parse for StringWireEnum {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let visibility: syn::Visibility = input.parse()?;
        let token: syn::Token![enum] = input.parse()?;
        let name: syn::Ident = input.parse()?;
        let body;
        syn::braced!(body in input);
        let mut variants = Vec::new();
        while !body.is_empty() {
            let attrs = body.call(Attribute::parse_outer)?;
            let name: syn::Ident = body.parse()?;
            let _: syn::Token![=>] = body.parse()?;
            let wire: syn::LitStr = body.parse()?;
            variants.push(quote::quote!(#(#attrs)* #[serde(rename = #wire)] #name));
            if !body.is_empty() {
                let _: syn::Token![,] = body.parse()?;
            }
        }
        Ok(Self(syn::parse2(
            quote::quote!(#(#attrs)* #visibility #token #name { #(#variants,)* }),
        )?))
    }
}
