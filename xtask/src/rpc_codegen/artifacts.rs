//! 稳定字段编号，以及现有 schema fixture 驱动的真实 DTO 往返验证。

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use kanban_protocol::HttpTransportLocation as Location;

use super::{Operation, write};
use crate::ToolResult;

pub type FieldNumbers = BTreeMap<String, BTreeMap<String, u32>>;

pub fn stabilize(source: &str, ledger: &mut FieldNumbers) -> ToolResult<String> {
    let mut output = String::new();
    let mut current: Option<String> = None;
    let mut depth = 0isize;
    let mut present = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if depth == 0 && (trimmed.starts_with("message ") || trimmed.starts_with("enum ")) {
            current = Some(
                trimmed
                    .split_whitespace()
                    .nth(1)
                    .ok_or("缺少 protobuf item name")?
                    .into(),
            );
            present.clear();
        }
        let mut rendered = line.to_owned();
        if let Some(item) = &current {
            if let Some((left, right)) = line.split_once(" = ")
                && let Ok(original) = right.trim().trim_end_matches(';').parse::<u32>()
            {
                let field = left
                    .split_whitespace()
                    .last()
                    .ok_or("缺少 protobuf field name")?;
                let tags = ledger.entry(item.clone()).or_default();
                let next = tags.values().max().map(|n| n + 1).unwrap_or(original);
                let index = *tags.entry(field.into()).or_insert(next);
                present.insert(field.to_owned());
                rendered = format!("{left} = {index};");
            }
            if depth == 1
                && trimmed == "}"
                && let Some(tags) = ledger.get(item)
            {
                let retired = tags
                    .iter()
                    .filter(|(name, _)| !present.contains(*name))
                    .collect::<Vec<_>>();
                if !retired.is_empty() {
                    output.push_str(&format!(
                        "  reserved {};\n",
                        retired
                            .iter()
                            .map(|(_, tag)| tag.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    output.push_str(&format!(
                        "  reserved {};\n",
                        retired
                            .iter()
                            .map(|(name, _)| format!("{name:?}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
        }
        output.push_str(&rendered);
        output.push('\n');
        if !trimmed.starts_with("//") {
            depth += line.matches('{').count() as isize - line.matches('}').count() as isize;
        }
        if depth == 0 {
            current = None;
        }
    }
    Ok(output)
}

pub fn fixture_tests(owner: &Path, operations: &[Operation], check: bool) -> ToolResult<()> {
    let catalog = kanban_protocol::operation_catalog();
    let mut source = String::from(
        "// @generated: 既有 contract valid fixtures 的 DTO -> proto bytes -> DTO 往返。\nuse kanban_protocol::rpc::v1;\nuse prost::Message;\n",
    );
    for operation in operations {
        let Some(parent) = catalog
            .iter()
            .find(|parent| parent.operation_id == operation.id)
        else {
            continue;
        };
        let name = super::model::snake(&operation.method);
        let mut parts = Vec::new();
        let mut setup = String::new();
        for (name, ty, location) in [
            ("path", &operation.path, Location::Path),
            ("query", &operation.query, Location::Query),
            ("input", &operation.input, Location::Body),
        ] {
            let Some(ty) = ty else {
                setup.push_str(&format!("let {name}=();\n"));
                parts.push(name);
                continue;
            };
            let original = parent
                .contracts
                .iter()
                .find(|c| c.location == Some(location))
                .and_then(|c| c.valid_fixture);
            let rust = ty.replace("crate::", "kanban_protocol::");
            if let Some(fixture) = original {
                setup.push_str(&format!("let {name}:{rust}=serde_json::from_str(include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"),\"/../../{fixture}\"))).unwrap();\n"));
            } else {
                setup.push_str(&format!(
                    "let {name}:{rust}=serde_json::from_str(\"{{}}\").unwrap();\n"
                ));
            }
            parts.push(name);
        }
        let request = format!("{}Request", operation.method);
        source.push_str(&format!("#[test] fn {name}_request_fixture_roundtrip(){{{setup}let expected=serde_json::to_value((&path,&query,&input)).unwrap();let wire=v1::{request}::from_parts({}).unwrap();let wire=v1::{request}::decode(wire.encode_to_vec().as_slice()).unwrap();let actual=wire.decode_parts().unwrap();assert_eq!(serde_json::to_value(actual).unwrap(),expected);}}\n",parts.join(",")));
        if matches!(
            operation.method.as_str(),
            "DownloadAttachment" | "QueryLabelAtomIndex"
        ) {
            continue;
        }
        let Some(fixture) = parent
            .contracts
            .iter()
            .find(|c| c.location == Some(Location::Success))
            .and_then(|c| c.valid_fixture)
        else {
            continue;
        };
        let rust = operation.response.replace("crate::", "kanban_protocol::");
        let response = format!("{}Response", operation.method);
        source.push_str(&format!("#[test] fn {name}_response_fixture_roundtrip(){{let dto:{rust}=serde_json::from_str(include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"),\"/../../{fixture}\"))).unwrap();let expected=serde_json::to_value(&dto).unwrap();let wire=v1::{response}::try_from(dto).unwrap();let wire=v1::{response}::decode(wire.encode_to_vec().as_slice()).unwrap();let actual:{rust}=wire.try_into().unwrap();assert_eq!(serde_json::to_value(actual).unwrap(),expected);}}\n"));
    }
    let source = super::format_rust(&prettyplease::unparse(&syn::parse_file(&source)?))?;
    if !check {
        fs::create_dir_all(owner.join("tests/rpc"))?;
    }
    write(&owner.join("tests/rpc/fixtures.rs"), &source, check)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorder_and_removal_preserve_tags() {
        let mut ledger = FieldNumbers::new();
        stabilize(
            "message A {\n string first = 1;\n string second = 2;\n}\n",
            &mut ledger,
        )
        .unwrap();
        let after = stabilize(
            "message A {\n string third = 1;\n string first = 2;\n}\n",
            &mut ledger,
        )
        .unwrap();
        assert!(after.contains("third = 3"));
        assert!(after.contains("first = 1"));
        assert!(after.contains("reserved 2;"));
        assert!(after.contains("reserved \"second\";"));
    }
}
