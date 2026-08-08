//! Cargo metadata 驱动的依赖 ownership、单主机和 schema tooling gate。
//!
//! 这里故意只检查 metadata/resolve 中会影响产品边界的字段。完整 workspace
//! dependency、target、registry checksum 镜像仍由 Cargo 自己维护，避免 gate
//! 复制第二份易漂移的规格。

use std::{
    collections::{HashMap, HashSet},
    fmt, fs,
    path::Path,
    process::Command,
};

use serde_json::{Map, Value};
use xtask::ToolResult;

use crate::process::status_description;

const CRATES_IO_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";
const TOOL_PACKAGE: &str = "xtask";
const CONTRACT_PACKAGE: &str = "kanban-protocol";
const WEB_ARTIFACT_PACKAGE: &str = "kanban-web-artifact";
const SERVICE_PACKAGE: &str = "kanban-service";
const SERVER_PACKAGE: &str = "kanban-server";
const JSONSCHEMA_PACKAGE: &str = "jsonschema";
const SCHEMARS_PACKAGE: &str = "schemars";
const FS4_PACKAGE: &str = "fs4";

const RETIRED_PACKAGES: &[&str] = &["kanban-sqlite", "kanban-local"];

#[derive(Clone, Copy)]
struct DependencyPolicy {
    name: &'static str,
    owner: &'static str,
    requirement: &'static str,
    exact_version: Option<&'static str>,
    uses_default_features: bool,
    features: &'static [&'static str],
}

const OWNER_POLICIES: &[DependencyPolicy] = &[
    DependencyPolicy {
        name: "turso",
        owner: SERVICE_PACKAGE,
        requirement: "=0.7.2",
        exact_version: Some("0.7.2"),
        uses_default_features: false,
        features: &["fts"],
    },
    DependencyPolicy {
        name: "axum",
        owner: SERVER_PACKAGE,
        requirement: "^0.7",
        exact_version: None,
        uses_default_features: true,
        features: &[],
    },
    DependencyPolicy {
        name: "ureq",
        owner: "kanban-client",
        requirement: "^2.12",
        exact_version: None,
        uses_default_features: false,
        features: &["json"],
    },
    DependencyPolicy {
        name: "rmcp",
        owner: "kanban-mcp",
        requirement: "=3.1.0",
        exact_version: Some("3.1.0"),
        uses_default_features: false,
        features: &["macros", "server", "transport-io"],
    },
    DependencyPolicy {
        name: "tauri",
        owner: "kanban-desktop",
        requirement: "^2",
        exact_version: None,
        uses_default_features: true,
        features: &["tray-icon"],
    },
    DependencyPolicy {
        name: "libc",
        owner: WEB_ARTIFACT_PACKAGE,
        requirement: "^0.2",
        exact_version: None,
        uses_default_features: false,
        features: &[],
    },
];

const TOOL_DEPENDENCIES: &[&str] = &[
    FS4_PACKAGE,
    JSONSCHEMA_PACKAGE,
    CONTRACT_PACKAGE,
    WEB_ARTIFACT_PACKAGE,
    "serde",
    "serde_json",
    "sha2",
];

const CONTRACT_DEPENDENCIES: &[&str] = &[SCHEMARS_PACKAGE, "serde", "serde_json", "sha2"];
const WEB_ARTIFACT_DEPENDENCIES: &[&str] = &[CONTRACT_PACKAGE, "libc"];
const WEB_ARTIFACT_DEV_DEPENDENCIES: &[&str] = &["serde_json", "tempfile"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct PolicyError(String);

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PolicyError {}

type PolicyResult<T> = Result<T, PolicyError>;
type PackageMap<'a> = HashMap<String, &'a Map<String, Value>>;

fn error(message: impl Into<String>) -> PolicyError {
    PolicyError(message.into())
}

fn object<'a>(value: &'a Value, context: &str) -> PolicyResult<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| error(format!("{context} 必须是 JSON object")))
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> PolicyResult<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| error(format!("{context} 缺少非空字符串字段 {field}")))
}

fn optional_string_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> PolicyResult<Option<&'a str>> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .filter(|value| !value.is_empty())
            .map(Some)
            .ok_or_else(|| error(format!("{context} 字段 {field} 必须是字符串或 null"))),
    }
}

fn bool_field(object: &Map<String, Value>, field: &str, context: &str) -> PolicyResult<bool> {
    object
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| error(format!("{context} 缺少布尔字段 {field}")))
}

fn string_array<'a>(value: &'a Value, context: &str) -> PolicyResult<Vec<&'a str>> {
    let array = value
        .as_array()
        .ok_or_else(|| error(format!("{context} 必须是 string array")))?;
    let mut values = Vec::with_capacity(array.len());
    for item in array {
        let item = item
            .as_str()
            .filter(|item| !item.is_empty())
            .ok_or_else(|| error(format!("{context} 包含无效 feature/name")))?;
        values.push(item);
    }
    Ok(values)
}

fn unique_string_array<'a>(value: &'a Value, context: &str) -> PolicyResult<Vec<&'a str>> {
    let values = string_array(value, context)?;
    let mut seen = HashSet::new();
    for value in &values {
        if !seen.insert(*value) {
            return Err(error(format!("{context} 存在重复项: {value}")));
        }
    }
    Ok(values)
}

fn package_name(package: &Map<String, Value>) -> PolicyResult<&str> {
    string_field(package, "name", "cargo metadata package")
}

fn package_id(package: &Map<String, Value>) -> PolicyResult<&str> {
    string_field(package, "id", "cargo metadata package")
}

fn dependency_identity(dependency: &Map<String, Value>) -> PolicyResult<&str> {
    // Cargo metadata v1 通常把真实 package 放在 `name`，把 leaf alias 放在
    // `rename`；兼容带显式 `package` 字段的 fixture，始终以 package identity
    // 做 owner/resolve 匹配。
    let name = string_field(dependency, "name", "cargo metadata dependency")?;
    if let Some(package) = dependency.get("package") {
        let package = package
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| error("cargo metadata dependency.package 必须是非空字符串"))?;
        return Ok(package);
    }
    Ok(name)
}

fn dependency_alias(dependency: &Map<String, Value>) -> PolicyResult<&str> {
    let name = string_field(dependency, "name", "cargo metadata dependency")?;
    match dependency.get("rename") {
        None | Some(Value::Null) => Ok(name),
        Some(value) => value
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| error("cargo metadata dependency.rename 必须是非空字符串或 null")),
    }
}

fn normal_kind(kind: Option<&str>) -> bool {
    kind.is_none() || kind == Some("normal")
}

fn dep_kind_matches(
    dependency: &Map<String, Value>,
    edge_kind: &Map<String, Value>,
) -> PolicyResult<bool> {
    let kind = optional_string_field(dependency, "kind", "cargo metadata dependency")?;
    let target = match dependency.get("target") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| error("cargo metadata dependency.target 必须是字符串或 null"))?,
        ),
    };
    let edge_kind_value = Value::Object(edge_kind.clone());
    let edge_kind = object(&edge_kind_value, "resolve dep_kinds item")?;
    let edge_kind_value = optional_string_field(edge_kind, "kind", "resolve dep_kinds item")?;
    let edge_target = optional_string_field(edge_kind, "target", "resolve dep_kinds item")?;
    Ok(
        (normal_kind(kind) && edge_kind_value.is_none() || kind == edge_kind_value)
            && target == edge_target,
    )
}

fn validate_dependency_shape(dependency: &Map<String, Value>, context: &str) -> PolicyResult<()> {
    dependency_identity(dependency)?;
    dependency_alias(dependency)?;
    optional_string_field(dependency, "source", context)?;
    optional_string_field(dependency, "req", context)?;
    optional_string_field(dependency, "kind", context)?;
    optional_string_field(dependency, "target", context)?;
    match dependency.get("registry") {
        None | Some(Value::Null) => {}
        Some(value) if value.as_str().is_some_and(|value| !value.is_empty()) => {}
        Some(_) => return Err(error(format!("{context} registry 必须是字符串或 null"))),
    }
    let optional = bool_field(dependency, "optional", context)?;
    let _ = optional;
    bool_field(dependency, "uses_default_features", context)?;
    string_array(
        dependency
            .get("features")
            .ok_or_else(|| error(format!("{context} 缺少 features")))?,
        context,
    )?;
    Ok(())
}

fn package_maps(metadata: &Value) -> PolicyResult<(PackageMap<'_>, Vec<String>)> {
    let metadata = object(metadata, "cargo metadata")?;
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| error("cargo metadata 缺少 packages array"))?;
    let mut by_id = HashMap::new();
    for package in packages {
        let package = object(package, "cargo metadata package")?;
        let id = package_id(package)?.to_owned();
        if by_id.insert(id.clone(), package).is_some() {
            return Err(error(format!("cargo metadata package id 重复: {id}")));
        }
        let name = package_name(package)?;
        let version = string_field(package, "version", "cargo metadata package")?;
        if let Some(source) = optional_string_field(package, "source", "cargo metadata package")? {
            let expected = format!("{source}#{name}@{version}");
            if id != expected {
                return Err(error(format!(
                    "package identity 与 source/name/version 不一致: expected={expected}, actual={id}"
                )));
            }
        } else if !id.contains('#') {
            return Err(error(format!(
                "workspace package id 缺少 identity separator: {id}"
            )));
        }
        let dependencies = package
            .get("dependencies")
            .and_then(Value::as_array)
            .ok_or_else(|| error(format!("{name} 缺少 dependencies array")))?;
        for dependency in dependencies {
            let dependency = object(dependency, "cargo metadata dependency")?;
            let context = format!("{name} dependency");
            validate_dependency_shape(dependency, &context)?;
        }
    }

    let workspace_members = unique_string_array(
        metadata
            .get("workspace_members")
            .ok_or_else(|| error("cargo metadata 缺少 workspace_members"))?,
        "workspace_members",
    )?;
    if workspace_members.is_empty() {
        return Err(error("workspace_members 不能为空"));
    }
    let mut seen_members = HashSet::new();
    for member in &workspace_members {
        if !seen_members.insert(*member) {
            return Err(error(format!(
                "workspace_members 存在重复 package id: {member}"
            )));
        }
        if !by_id.contains_key(*member) {
            return Err(error(format!(
                "workspace member 缺少 package record: {member}"
            )));
        }
    }

    let default_members = unique_string_array(
        metadata
            .get("workspace_default_members")
            .ok_or_else(|| error("cargo metadata 缺少 workspace_default_members"))?,
        "workspace_default_members",
    )?;
    let member_set = workspace_members.iter().copied().collect::<HashSet<_>>();
    for member in default_members {
        if !member_set.contains(member) {
            return Err(error(format!(
                "workspace_default_members 包含非 workspace package: {member}"
            )));
        }
    }
    let default_members = metadata
        .get("workspace_default_members")
        .and_then(Value::as_array)
        .expect("validated workspace_default_members");
    Ok((
        by_id,
        default_members
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
    ))
}

fn resolve_maps<'a>(
    metadata: &'a Value,
    packages: &PackageMap<'a>,
) -> PolicyResult<PackageMap<'a>> {
    let metadata = object(metadata, "cargo metadata")?;
    let resolve = object(
        metadata
            .get("resolve")
            .ok_or_else(|| error("cargo metadata 缺少 resolve"))?,
        "cargo metadata resolve",
    )?;
    let nodes = resolve
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| error("cargo metadata resolve 缺少 nodes array"))?;
    let mut by_id = HashMap::new();
    for node in nodes {
        let node = object(node, "cargo metadata resolve node")?;
        let id = string_field(node, "id", "cargo metadata resolve node")?.to_owned();
        if !packages.contains_key(&id) {
            return Err(error(format!("resolve node 指向未知 package: {id}")));
        }
        if by_id.insert(id.clone(), node).is_some() {
            return Err(error(format!("resolve node id 重复: {id}")));
        }
        let dependencies = node
            .get("dependencies")
            .and_then(Value::as_array)
            .ok_or_else(|| error(format!("resolve node {id} 缺少 dependencies")))?;
        let edges = node
            .get("deps")
            .and_then(Value::as_array)
            .ok_or_else(|| error(format!("resolve node {id} 缺少 deps")))?;
        unique_string_array(
            node.get("features")
                .ok_or_else(|| error(format!("resolve node {id} 缺少 features")))?,
            &format!("resolve node {id} features"),
        )?;
        if dependencies.len() != edges.len()
            || dependencies.iter().any(|dependency| {
                let Some(package_id) = dependency.as_str() else {
                    return true;
                };
                !edges.iter().any(|edge| {
                    edge.as_object()
                        .and_then(|edge| edge.get("pkg"))
                        .and_then(Value::as_str)
                        == Some(package_id)
                })
            })
        {
            return Err(error(format!(
                "resolve node {id} dependencies 与 deps[].pkg 映射不一致"
            )));
        }

        let package = packages
            .get(&id)
            .ok_or_else(|| error(format!("resolve node package record 缺失: {id}")))?;
        let owner_name = package_name(package)?;
        let declared = package
            .get("dependencies")
            .and_then(Value::as_array)
            .ok_or_else(|| error(format!("{owner_name} 缺少 dependencies")))?;
        for edge in edges {
            let edge = object(edge, "resolve dependency edge")?;
            let alias = string_field(edge, "name", "resolve dependency edge")?;
            let package_id = string_field(edge, "pkg", "resolve dependency edge")?;
            if !packages.contains_key(package_id) {
                return Err(error(format!(
                    "{owner_name} resolve edge 指向未知 package: {package_id}"
                )));
            }
            let dep_kinds = edge
                .get("dep_kinds")
                .and_then(Value::as_array)
                .ok_or_else(|| error(format!("{owner_name} resolve edge 缺少 dep_kinds")))?;
            if dep_kinds.is_empty() {
                return Err(error(format!(
                    "{owner_name} resolve edge {alias} dep_kinds 不能为空"
                )));
            }
            for kind in dep_kinds {
                let kind = object(kind, "resolve dep_kinds item")?;
                if kind.len() != 2 || !kind.contains_key("kind") || !kind.contains_key("target") {
                    return Err(error("resolve dep_kinds item 必须精确包含 kind/target"));
                }
                let kind_value = optional_string_field(kind, "kind", "resolve dep_kinds item")?;
                if kind_value.is_some_and(|value| !matches!(value, "dev" | "build" | "normal")) {
                    return Err(error(format!(
                        "resolve dep_kinds kind 无效: {kind_value:?}"
                    )));
                }
                optional_string_field(kind, "target", "resolve dep_kinds item")?;
            }
            let resolved_package_for_alias = packages
                .get(package_id)
                .ok_or_else(|| error(format!("resolve edge package 缺失: {package_id}")))?;
            let declared = declared
                .iter()
                .filter_map(Value::as_object)
                .filter(|dependency| {
                    dependency_alias(dependency).is_ok_and(|declared_alias| {
                        declared_alias == alias
                            || declared_alias.replace('-', "_") == alias
                            || (dependency.get("rename").is_none_or(Value::is_null)
                                && package_target_names(resolved_package_for_alias)
                                    .contains(&alias))
                    })
                })
                .collect::<Vec<_>>();
            if declared.is_empty() {
                return Err(error(format!(
                    "{owner_name} resolve edge alias 没有 metadata declaration: {alias}"
                )));
            }
            let resolved_package = packages
                .get(package_id)
                .ok_or_else(|| error(format!("resolve edge package 缺失: {package_id}")))?;
            let resolved_name = package_name(resolved_package)?;
            let resolved_source = optional_string_field(
                resolved_package,
                "source",
                "cargo metadata resolved package",
            )?;
            let matched = declared.into_iter().find(|declared| {
                let identity_matches = dependency_identity(declared)
                    .ok()
                    .is_some_and(|identity| identity == resolved_name);
                let source_matches =
                    optional_string_field(declared, "source", "cargo metadata dependency")
                        .ok()
                        .flatten()
                        == resolved_source;
                let kind_matches = dep_kinds.iter().any(|kind| {
                    kind.as_object()
                        .and_then(|kind| dep_kind_matches(declared, kind).ok())
                        .unwrap_or(false)
                });
                identity_matches && source_matches && kind_matches
            });
            if matched.is_none() {
                return Err(error(format!(
                    "{owner_name} resolve edge {alias} identity/source/dep_kinds 与 declaration 不一致"
                )));
            }
        }
    }
    if by_id.len() != packages.len() {
        return Err(error(format!(
            "resolve nodes 未覆盖全部 package records: nodes={}, packages={}",
            by_id.len(),
            packages.len()
        )));
    }
    Ok(by_id)
}

fn workspace_by_name(
    metadata: &Value,
    packages: &PackageMap<'_>,
) -> PolicyResult<HashMap<String, String>> {
    let metadata = object(metadata, "cargo metadata")?;
    let members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or_else(|| error("cargo metadata 缺少 workspace_members"))?;
    let mut by_name = HashMap::new();
    for member in members {
        let id = member
            .as_str()
            .ok_or_else(|| error("workspace_members 必须是 string array"))?;
        let package = packages
            .get(id)
            .ok_or_else(|| error(format!("workspace member 缺少 package: {id}")))?;
        let name = package_name(package)?.to_owned();
        if by_name.insert(name.clone(), id.to_owned()).is_some() {
            return Err(error(format!("workspace package name 重复: {name}")));
        }
    }
    Ok(by_name)
}

fn package_dependencies(package: &Map<String, Value>) -> PolicyResult<Vec<&Map<String, Value>>> {
    package
        .get("dependencies")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            error(format!(
                "{} 缺少 dependencies",
                package_name(package).unwrap_or("package")
            ))
        })?
        .iter()
        .map(|dependency| object(dependency, "cargo metadata dependency"))
        .collect()
}

fn package_target_names(package: &Map<String, Value>) -> Vec<&str> {
    package
        .get("targets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .filter_map(|target| target.get("name").and_then(Value::as_str))
        .collect()
}

fn direct_edge<'a>(
    node: &'a Map<String, Value>,
    alias: &str,
    owner: &str,
) -> PolicyResult<&'a Map<String, Value>> {
    let edges = node
        .get("deps")
        .and_then(Value::as_array)
        .ok_or_else(|| error(format!("{owner} resolve node 缺少 deps")))?;
    let matches = edges
        .iter()
        .filter_map(Value::as_object)
        .filter(|edge| edge.get("name").and_then(Value::as_str) == Some(alias))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(error(format!(
            "{owner} resolved direct edge {alias} 必须唯一，实际 {}",
            matches.len()
        )));
    }
    Ok(matches[0])
}

fn normal_edge(edge: &Map<String, Value>, context: &str, exact_only: bool) -> PolicyResult<()> {
    let kinds = edge
        .get("dep_kinds")
        .and_then(Value::as_array)
        .ok_or_else(|| error(format!("{context} 缺少 dep_kinds")))?;
    let normal = kinds.iter().filter_map(Value::as_object).filter(|kind| {
        optional_string_field(kind, "kind", context)
            .ok()
            .flatten()
            .is_none()
            && optional_string_field(kind, "target", context)
                .ok()
                .flatten()
                .is_none()
    });
    if exact_only {
        if kinds.len() != 1 || normal.count() != 1 {
            return Err(error(format!(
                "{context} 必须是唯一 unconditional normal edge"
            )));
        }
    } else if normal.count() == 0 {
        return Err(error(format!("{context} 缺少 unconditional normal edge")));
    }
    Ok(())
}

fn dev_edge(edge: &Map<String, Value>, context: &str) -> PolicyResult<()> {
    let kinds = edge
        .get("dep_kinds")
        .and_then(Value::as_array)
        .ok_or_else(|| error(format!("{context} 缺少 dep_kinds")))?;
    if kinds.len() != 1 {
        return Err(error(format!("{context} 必须是唯一 dev edge")));
    }
    let kind = kinds[0]
        .as_object()
        .ok_or_else(|| error(format!("{context} dev edge kind 必须是 object")))?;
    if optional_string_field(kind, "kind", context)? != Some("dev")
        || optional_string_field(kind, "target", context)?.is_some()
    {
        return Err(error(format!("{context} 必须是 unconditional dev edge")));
    }
    Ok(())
}

fn feature_set<'a>(
    dependency: &'a Map<String, Value>,
    context: &str,
) -> PolicyResult<HashSet<&'a str>> {
    Ok(unique_string_array(
        dependency
            .get("features")
            .ok_or_else(|| error(format!("{context} 缺少 features")))?,
        context,
    )?
    .into_iter()
    .collect())
}

fn resolved_version_matches(version: &str, policy: &DependencyPolicy) -> bool {
    if let Some(exact) = policy.exact_version {
        return version == exact;
    }
    match policy.requirement {
        "^0.7" => version.starts_with("0.7."),
        "^0.2" => version.starts_with("0.2."),
        "^2.12" => version.starts_with("2.12."),
        "^2" => version.starts_with("2."),
        _ => false,
    }
}

fn validate_owner_policies(
    workspace: &HashMap<String, String>,
    packages: &HashMap<String, &Map<String, Value>>,
    nodes: &HashMap<String, &Map<String, Value>>,
) -> PolicyResult<()> {
    for policy in OWNER_POLICIES {
        let mut owners = Vec::new();
        for (name, id) in workspace {
            let package = packages
                .get(id)
                .ok_or_else(|| error(format!("workspace package 缺失: {id}")))?;
            for dependency in package_dependencies(package)? {
                if dependency_identity(dependency)? == policy.name {
                    owners.push((name.as_str(), *package, dependency));
                }
            }
        }
        if owners.len() != 1 || owners[0].0 != policy.owner {
            return Err(error(format!(
                "{} 必须只有 owner {}，实际 owners={:?}",
                policy.name,
                policy.owner,
                owners.iter().map(|owner| owner.0).collect::<Vec<_>>()
            )));
        }
        let (owner_name, owner_package, dependency) = owners[0];
        let context = format!("{owner_name} -> {}", policy.name);
        if dependency_alias(dependency)? != policy.name {
            return Err(error(format!("{context} 禁止 dependency alias rename")));
        }
        if dependency.get("req").and_then(Value::as_str) != Some(policy.requirement) {
            return Err(error(format!(
                "{context} req 必须是 {}，实际 {:?}",
                policy.requirement,
                dependency.get("req")
            )));
        }
        if optional_string_field(dependency, "source", &context)? != Some(CRATES_IO_SOURCE) {
            return Err(error(format!("{context} source 必须是 crates.io")));
        }
        if dependency.get("path").is_some()
            || dependency
                .get("registry")
                .is_some_and(|value| !value.is_null())
        {
            return Err(error(format!("{context} 禁止 path/registry override")));
        }
        if dependency
            .get("kind")
            .is_some_and(|value| value.as_str().is_some_and(|kind| !normal_kind(Some(kind))))
        {
            return Err(error(format!("{context} 必须是 normal dependency")));
        }
        if !normal_kind(dependency.get("kind").and_then(Value::as_str))
            || !matches!(dependency.get("optional"), Some(Value::Bool(false)))
            || dependency
                .get("target")
                .is_some_and(|value| !value.is_null())
        {
            return Err(error(format!(
                "{context} 必须是 nonoptional、非 target normal dependency"
            )));
        }
        let actual_features = feature_set(dependency, &context)?;
        let expected_features = policy.features.iter().copied().collect::<HashSet<_>>();
        if actual_features != expected_features {
            return Err(error(format!(
                "{context} leaf features 漂移: expected={expected_features:?}, actual={actual_features:?}"
            )));
        }
        if bool_field(dependency, "uses_default_features", &context)?
            != policy.uses_default_features
        {
            return Err(error(format!("{context} default feature policy 漂移")));
        }

        let owner_id = workspace
            .get(owner_name)
            .ok_or_else(|| error(format!("workspace owner 缺失: {owner_name}")))?;
        let node = nodes
            .get(owner_id)
            .ok_or_else(|| error(format!("{owner_name} 缺少 resolve node")))?;
        let edge = direct_edge(node, policy.name, owner_name)?;
        normal_edge(edge, &context, true)?;
        let resolved_id = string_field(edge, "pkg", &context)?;
        let resolved = packages
            .get(resolved_id)
            .ok_or_else(|| error(format!("{context} resolved package 缺失: {resolved_id}")))?;
        if package_name(resolved)? != policy.name
            || optional_string_field(resolved, "source", &context)? != Some(CRATES_IO_SOURCE)
        {
            return Err(error(format!(
                "{context} resolved package identity/source 错误"
            )));
        }
        let version = string_field(resolved, "version", &context)?;
        if !resolved_version_matches(version, policy) {
            return Err(error(format!(
                "{context} resolved version 不满足 {}: {version}",
                policy.requirement
            )));
        }
        let resolved_node = nodes
            .get(resolved_id)
            .ok_or_else(|| error(format!("{context} resolved node 缺失")))?;
        let features = unique_string_array(
            resolved_node
                .get("features")
                .ok_or_else(|| error(format!("{context} resolved features 缺失")))?,
            &format!("{context} resolved features"),
        )?
        .into_iter()
        .collect::<HashSet<_>>();
        if policy.uses_default_features && !features.contains("default") {
            return Err(error(format!("{context} resolved default feature 漂移")));
        }
        if !policy
            .features
            .iter()
            .all(|feature| features.contains(feature))
        {
            return Err(error(format!(
                "{context} resolved features 缺少 owner feature"
            )));
        }

        // 即使 fixture 通过额外 metadata 字段设置 alias，也保持 owner package
        // identity 的显式绑定。
        if package_name(owner_package)? != policy.owner {
            return Err(error(format!(
                "workspace owner identity 漂移: {owner_name}"
            )));
        }
    }
    Ok(())
}

fn validate_single_host(
    workspace: &HashMap<String, String>,
    packages: &HashMap<String, &Map<String, Value>>,
    nodes: &HashMap<String, &Map<String, Value>>,
) -> PolicyResult<()> {
    for (name, id) in workspace {
        let package = packages
            .get(id)
            .ok_or_else(|| error(format!("workspace package 缺失: {id}")))?;
        for dependency in package_dependencies(package)? {
            let identity = dependency_identity(dependency)?;
            if identity != SERVICE_PACKAGE && identity != "turso" {
                continue;
            }
            if identity == SERVICE_PACKAGE && name != SERVER_PACKAGE {
                return Err(error(format!(
                    "{name} 禁止直接依赖 {SERVICE_PACKAGE}，single-host 方向必须是 server -> service"
                )));
            }
            if identity == "turso" && name != SERVICE_PACKAGE {
                return Err(error(format!(
                    "{name} 禁止直接依赖 turso，single-host 方向必须是 service -> turso"
                )));
            }
            if identity == SERVICE_PACKAGE && dependency_alias(dependency)? != SERVICE_PACKAGE {
                return Err(error("kanban-server -> kanban-service 禁止 alias"));
            }
            let owner_id = workspace
                .get(name)
                .ok_or_else(|| error(format!("workspace package 缺失: {name}")))?;
            let node = nodes
                .get(owner_id)
                .ok_or_else(|| error(format!("{name} 缺少 resolve node")))?;
            let edge = direct_edge(node, &identity.replace('-', "_"), name)
                .or_else(|_| direct_edge(node, identity, name))?;
            normal_edge(edge, &format!("{name} -> {identity}"), false)?;
            let resolved = packages
                .get(string_field(edge, "pkg", "single-host edge")?)
                .ok_or_else(|| error("single-host edge resolved package 缺失"))?;
            if package_name(resolved)? != identity {
                return Err(error(format!("single-host edge identity 错误: {identity}")));
            }
        }
    }
    Ok(())
}

fn is_normal_edge(edge: &Map<String, Value>) -> bool {
    edge.get("dep_kinds")
        .and_then(Value::as_array)
        .is_some_and(|kinds| {
            kinds.iter().any(|kind| {
                kind.as_object().is_some_and(|kind| {
                    optional_string_field(kind, "kind", "resolve dep_kinds")
                        .ok()
                        .flatten()
                        .is_none()
                        && optional_string_field(kind, "target", "resolve dep_kinds")
                            .ok()
                            .flatten()
                            .is_none()
                })
            })
        })
}

fn reachable_normal(
    root_id: &str,
    packages: &HashMap<String, &Map<String, Value>>,
    nodes: &HashMap<String, &Map<String, Value>>,
) -> PolicyResult<HashSet<String>> {
    let mut visited = HashSet::new();
    let mut pending = vec![root_id.to_owned()];
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let node = nodes
            .get(&id)
            .ok_or_else(|| error(format!("reachable graph 缺少 resolve node: {id}")))?;
        let edges = node
            .get("deps")
            .and_then(Value::as_array)
            .ok_or_else(|| error(format!("reachable graph node 缺少 deps: {id}")))?;
        for edge in edges
            .iter()
            .filter_map(Value::as_object)
            .filter(|edge| is_normal_edge(edge))
        {
            let package_id = string_field(edge, "pkg", "reachable resolve edge")?;
            if !packages.contains_key(package_id) {
                return Err(error(format!(
                    "reachable graph 指向未知 package: {package_id}"
                )));
            }
            pending.push(package_id.to_owned());
        }
    }
    Ok(visited)
}

fn validate_runtime_isolation(
    metadata: &Value,
    workspace: &HashMap<String, String>,
    default_members: &[String],
    packages: &HashMap<String, &Map<String, Value>>,
    nodes: &HashMap<String, &Map<String, Value>>,
) -> PolicyResult<()> {
    if default_members.iter().any(|id| {
        workspace
            .get(TOOL_PACKAGE)
            .is_some_and(|tool_id| id == tool_id)
    }) {
        return Err(error("workspace.default-members 禁止包含 xtask"));
    }

    let mcp_id = workspace
        .get("kanban-mcp")
        .ok_or_else(|| error("workspace 缺少 kanban-mcp"))?;
    let mcp = packages
        .get(mcp_id)
        .ok_or_else(|| error("kanban-mcp package 缺失"))?;
    let mcp_contract = package_dependencies(mcp)?
        .into_iter()
        .find(|dependency| dependency_identity(dependency).ok() == Some(CONTRACT_PACKAGE));
    let Some(mcp_contract) = mcp_contract else {
        return Err(error(
            "kanban-mcp 必须显式声明 kanban-protocol/schema runtime exception",
        ));
    };
    if mcp_contract.get("optional") != Some(&Value::Bool(false))
        || !normal_kind(mcp_contract.get("kind").and_then(Value::as_str))
        || mcp_contract
            .get("target")
            .is_some_and(|value| !value.is_null())
        || !matches!(
            mcp_contract.get("uses_default_features"),
            Some(Value::Bool(false))
        )
        || feature_set(mcp_contract, "kanban-mcp -> kanban-protocol")? != HashSet::from(["schema"])
    {
        return Err(error(
            "kanban-mcp -> kanban-protocol 必须是 default-features=false + schema",
        ));
    }
    let contract_id = workspace
        .get(CONTRACT_PACKAGE)
        .ok_or_else(|| error("workspace 缺少 kanban-protocol"))?;
    let contract = packages
        .get(contract_id)
        .ok_or_else(|| error("kanban-protocol package 缺失"))?;
    let contract_features = contract
        .get("features")
        .and_then(Value::as_object)
        .ok_or_else(|| error("kanban-protocol 缺少 features object"))?;
    if !unique_string_array(
        contract_features
            .get("default")
            .ok_or_else(|| error("kanban-protocol 缺少 default feature"))?,
        "kanban-protocol default feature",
    )?
    .is_empty()
        || unique_string_array(
            contract_features
                .get("schema")
                .ok_or_else(|| error("kanban-protocol 缺少 schema feature"))?,
            "kanban-protocol schema feature",
        )? != ["dep:schemars"]
    {
        return Err(error(
            "kanban-protocol features 必须精确为 default=[] 与 schema=[dep:schemars]",
        ));
    }

    // Cargo metadata 暴露的是统一 feature graph，因此 runtime schema exception
    // 由 canonical protocol edge 表示；其他 runtime leaf 仍不能直接请求 schema。
    for (name, id) in workspace {
        if name == TOOL_PACKAGE {
            continue;
        }
        let package = packages
            .get(id)
            .ok_or_else(|| error(format!("workspace package 缺失: {id}")))?;
        for dependency in package_dependencies(package)? {
            let identity = dependency_identity(dependency)?;
            if identity == TOOL_PACKAGE || identity == JSONSCHEMA_PACKAGE {
                return Err(error(format!(
                    "{name} 禁止直接依赖 schema tooling {identity}"
                )));
            }
            if identity == SCHEMARS_PACKAGE && name != CONTRACT_PACKAGE {
                return Err(error(format!(
                    "{name} 禁止直接依赖 schema-only {SCHEMARS_PACKAGE}"
                )));
            }
            if identity == CONTRACT_PACKAGE
                && name != "kanban-mcp"
                && name != TOOL_PACKAGE
                && feature_set(dependency, &format!("{name} -> {CONTRACT_PACKAGE}"))?
                    .contains("schema")
            {
                return Err(error(format!(
                    "{name} 禁止直接启用 {CONTRACT_PACKAGE}/schema exception"
                )));
            }
        }
        let reachable = reachable_normal(id, packages, nodes)?;
        for reachable_id in reachable {
            let reachable_package = packages
                .get(&reachable_id)
                .ok_or_else(|| error(format!("runtime graph package 缺失: {reachable_id}")))?;
            let reachable_name = package_name(reachable_package)?;
            if matches!(reachable_name, TOOL_PACKAGE | JSONSCHEMA_PACKAGE) {
                return Err(error(format!(
                    "{name} runtime/default graph 泄漏 schema tooling: {reachable_name}"
                )));
            }
        }
    }

    // 保留 metadata 参数，确保 fixture 也经过同一套 default-members 校验路径。
    let _ = metadata;
    Ok(())
}

fn validate_legacy_graph(
    workspace: &HashMap<String, String>,
    packages: &HashMap<String, &Map<String, Value>>,
    nodes: &HashMap<String, &Map<String, Value>>,
) -> PolicyResult<()> {
    for id in workspace.values() {
        for reachable_id in reachable_normal(id, packages, nodes)? {
            let package = packages
                .get(&reachable_id)
                .ok_or_else(|| error(format!("legacy graph package 缺失: {reachable_id}")))?;
            let name = package_name(package)?;
            if RETIRED_PACKAGES.contains(&name) {
                return Err(error(format!("active graph 禁止 legacy package: {name}")));
            }
            if name == "rusqlite" {
                // 当前 metadata 可能因 service test-support/legacy-sqlite-import
                // 显式 opt-in 而包含 importer；其他 owner 即使 Cargo resolve 了
                // registry package 仍然禁止持有它。
                let service_id = workspace
                    .get(SERVICE_PACKAGE)
                    .ok_or_else(|| error("workspace 缺少 kanban-service"))?;
                let service = packages
                    .get(service_id)
                    .ok_or_else(|| error("kanban-service package 缺失"))?;
                let allowed = package_dependencies(service)?
                    .into_iter()
                    .any(|dependency| {
                        dependency_identity(dependency).ok() == Some("rusqlite")
                            && dependency.get("optional") == Some(&Value::Bool(true))
                    });
                if !allowed {
                    return Err(error(
                        "rusqlite 只能由 kanban-service optional importer 持有",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_tool_and_contract(
    workspace: &HashMap<String, String>,
    packages: &HashMap<String, &Map<String, Value>>,
    nodes: &HashMap<String, &Map<String, Value>>,
) -> PolicyResult<()> {
    let tool_id = workspace
        .get(TOOL_PACKAGE)
        .ok_or_else(|| error("workspace 缺少 xtask"))?;
    let contract_id = workspace
        .get(CONTRACT_PACKAGE)
        .ok_or_else(|| error("workspace 缺少 kanban-protocol"))?;
    let tool = packages
        .get(tool_id)
        .ok_or_else(|| error("xtask package 缺失"))?;
    let contract = packages
        .get(contract_id)
        .ok_or_else(|| error("kanban-protocol package 缺失"))?;

    validate_exact_direct_dependencies(tool, TOOL_PACKAGE, TOOL_DEPENDENCIES)?;
    validate_exact_direct_dependencies(contract, CONTRACT_PACKAGE, CONTRACT_DEPENDENCIES)?;

    for dependency in package_dependencies(tool)? {
        let name = dependency_identity(dependency)?;
        let context = format!("xtask -> {name}");
        if dependency_alias(dependency)? != name
            || !normal_kind(dependency.get("kind").and_then(Value::as_str))
            || dependency.get("optional") != Some(&Value::Bool(false))
            || dependency
                .get("target")
                .is_some_and(|value| !value.is_null())
        {
            return Err(error(format!(
                "{context} 必须是唯一 normal nonoptional non-target edge"
            )));
        }
        match name {
            FS4_PACKAGE => check_registry_declaration(dependency, "^0.13.1", true, &[], &context)?,
            JSONSCHEMA_PACKAGE => {
                check_registry_declaration(dependency, "^0.47.0", false, &[], &context)?
            }
            CONTRACT_PACKAGE => {
                if dependency.get("source") != Some(&Value::Null)
                    || dependency.get("path").and_then(Value::as_str).is_none()
                    || bool_field(dependency, "uses_default_features", &context)?
                    || feature_set(dependency, &context)? != HashSet::from(["schema"])
                {
                    return Err(error(format!(
                        "{context} schema exception declaration 漂移"
                    )));
                }
            }
            WEB_ARTIFACT_PACKAGE => {
                if dependency.get("source") != Some(&Value::Null)
                    || dependency.get("path").and_then(Value::as_str).is_none()
                    || bool_field(dependency, "uses_default_features", &context)?
                    || !feature_set(dependency, &context)?.is_empty()
                {
                    return Err(error(format!(
                        "{context} 必须是 path + default-features=false 且不启用额外 feature"
                    )));
                }
            }
            "serde" => check_registry_declaration(dependency, "^1.0", true, &["derive"], &context)?,
            "serde_json" => check_registry_declaration(dependency, "^1.0", true, &[], &context)?,
            "sha2" => check_registry_declaration(dependency, "^0.10", true, &[], &context)?,
            _ => unreachable!("exact direct dependency list already checked"),
        }
    }
    for dependency in package_dependencies(contract)? {
        let name = dependency_identity(dependency)?;
        let context = format!("kanban-protocol -> {name}");
        match name {
            SCHEMARS_PACKAGE => check_registry_declaration(
                dependency,
                "^1.2.1",
                false,
                &["std", "derive"],
                &context,
            )?,
            "serde" => check_registry_declaration(dependency, "^1.0", true, &["derive"], &context)?,
            "serde_json" => check_registry_declaration(dependency, "^1.0", true, &[], &context)?,
            "sha2" => check_registry_declaration(dependency, "^0.10", true, &[], &context)?,
            _ => unreachable!("exact contract dependency list already checked"),
        }
        if name == SCHEMARS_PACKAGE && dependency.get("optional") != Some(&Value::Bool(true)) {
            return Err(error(
                "kanban-protocol -> schemars 必须保持 optional schema dependency",
            ));
        }
    }

    let tool_node = nodes
        .get(tool_id)
        .ok_or_else(|| error("xtask resolve node 缺失"))?;
    let contract_node = nodes
        .get(contract_id)
        .ok_or_else(|| error("kanban-protocol resolve node 缺失"))?;
    for dependency_name in TOOL_DEPENDENCIES {
        let alias = dependency_name.replace('-', "_");
        let edge = direct_edge(tool_node, &alias, TOOL_PACKAGE)?;
        normal_edge(edge, &format!("xtask -> {dependency_name}"), true)?;
        let resolved = packages
            .get(string_field(edge, "pkg", "xtask resolve edge")?)
            .ok_or_else(|| error("xtask resolve package 缺失"))?;
        if package_name(resolved)? != *dependency_name {
            return Err(error(format!(
                "xtask resolve edge identity 错误: {dependency_name}"
            )));
        }
    }
    for dependency_name in CONTRACT_DEPENDENCIES {
        let edge = direct_edge(
            contract_node,
            &dependency_name.replace('-', "_"),
            CONTRACT_PACKAGE,
        )?;
        normal_edge(edge, &format!("kanban-protocol -> {dependency_name}"), true)?;
    }
    let jsonschema_id = string_field(
        direct_edge(tool_node, JSONSCHEMA_PACKAGE, TOOL_PACKAGE)?,
        "pkg",
        "xtask jsonschema edge",
    )?;
    let jsonschema_node = nodes
        .get(jsonschema_id)
        .ok_or_else(|| error("jsonschema resolve node 缺失"))?;
    if !unique_string_array(
        jsonschema_node
            .get("features")
            .ok_or_else(|| error("jsonschema features 缺失"))?,
        "jsonschema features",
    )?
    .is_empty()
    {
        return Err(error(
            "jsonschema effective features 必须为空（no-default）",
        ));
    }
    let schemars_id = string_field(
        direct_edge(contract_node, SCHEMARS_PACKAGE, CONTRACT_PACKAGE)?,
        "pkg",
        "schemars edge",
    )?;
    let schemars = packages
        .get(schemars_id)
        .ok_or_else(|| error("schemars package 缺失"))?;
    if string_field(schemars, "version", "schemars")? != "1.2.1"
        || optional_string_field(schemars, "source", "schemars")? != Some(CRATES_IO_SOURCE)
    {
        return Err(error(
            "kanban-protocol/schema 必须 resolve schemars 1.2.1 crates.io",
        ));
    }
    let schemars_node = nodes
        .get(schemars_id)
        .ok_or_else(|| error("schemars resolve node 缺失"))?;
    let actual_schemars_features = unique_string_array(
        schemars_node
            .get("features")
            .ok_or_else(|| error("schemars features 缺失"))?,
        "schemars features",
    )?
    .into_iter()
    .collect::<HashSet<_>>();
    let expected_schemars_features =
        HashSet::from(["chrono04", "default", "derive", "schemars_derive", "std"]);
    if actual_schemars_features != expected_schemars_features {
        return Err(error(format!(
            "schemars effective features 漂移: expected={expected_schemars_features:?}, actual={actual_schemars_features:?}"
        )));
    }
    Ok(())
}

fn validate_web_artifact_dependencies(
    workspace: &HashMap<String, String>,
    packages: &HashMap<String, &Map<String, Value>>,
    nodes: &HashMap<String, &Map<String, Value>>,
) -> PolicyResult<()> {
    let package_id = workspace
        .get(WEB_ARTIFACT_PACKAGE)
        .ok_or_else(|| error("workspace 缺少 kanban-web-artifact"))?;
    let package = packages
        .get(package_id)
        .ok_or_else(|| error("kanban-web-artifact package 缺失"))?;
    for dependency in package_dependencies(package)? {
        let kind = dependency.get("kind").and_then(Value::as_str);
        if !normal_kind(kind) && kind != Some("dev") {
            return Err(error(format!(
                "{WEB_ARTIFACT_PACKAGE} 禁止未冻结 dependency kind: {kind:?}"
            )));
        }
    }
    validate_exact_normal_dependencies(package, WEB_ARTIFACT_PACKAGE, WEB_ARTIFACT_DEPENDENCIES)?;

    for dependency in package_dependencies(package)?
        .into_iter()
        .filter(|dependency| normal_kind(dependency.get("kind").and_then(Value::as_str)))
    {
        let name = dependency_identity(dependency)?;
        let context = format!("{WEB_ARTIFACT_PACKAGE} -> {name}");
        if dependency_alias(dependency)? != name
            || !normal_kind(dependency.get("kind").and_then(Value::as_str))
            || dependency.get("optional") != Some(&Value::Bool(false))
            || dependency
                .get("target")
                .is_some_and(|value| !value.is_null())
        {
            return Err(error(format!(
                "{context} 必须是唯一 normal nonoptional non-target edge"
            )));
        }
        match name {
            CONTRACT_PACKAGE => {
                if dependency.get("source") != Some(&Value::Null)
                    || dependency.get("path").and_then(Value::as_str).is_none()
                    || bool_field(dependency, "uses_default_features", &context)?
                    || !feature_set(dependency, &context)?.is_empty()
                {
                    return Err(error(format!(
                        "{context} 必须是 path + default-features=false 且不启用 schema"
                    )));
                }
            }
            "libc" => check_registry_declaration(dependency, "^0.2", false, &[], &context)?,
            _ => unreachable!("exact Web artifact dependency list already checked"),
        }
    }

    validate_exact_dev_dependencies(package, WEB_ARTIFACT_PACKAGE, WEB_ARTIFACT_DEV_DEPENDENCIES)?;
    for dependency in package_dependencies(package)?
        .into_iter()
        .filter(|dependency| dependency.get("kind").and_then(Value::as_str) == Some("dev"))
    {
        let name = dependency_identity(dependency)?;
        let context = format!("{WEB_ARTIFACT_PACKAGE} dev -> {name}");
        if dependency_alias(dependency)? != name
            || dependency.get("optional") != Some(&Value::Bool(false))
            || dependency
                .get("target")
                .is_some_and(|value| !value.is_null())
        {
            return Err(error(format!(
                "{context} 必须是 nonoptional、非 target dev dependency"
            )));
        }
        match name {
            "serde_json" => check_registry_declaration(dependency, "^1.0", true, &[], &context)?,
            "tempfile" => check_registry_declaration(dependency, "^3.10", true, &[], &context)?,
            _ => unreachable!("exact Web artifact dev dependency list already checked"),
        }
    }

    let node = nodes
        .get(package_id)
        .ok_or_else(|| error("kanban-web-artifact resolve node 缺失"))?;
    for dependency_name in WEB_ARTIFACT_DEPENDENCIES {
        let edge = direct_edge(
            node,
            &dependency_name.replace('-', "_"),
            WEB_ARTIFACT_PACKAGE,
        )?;
        normal_edge(
            edge,
            &format!("{WEB_ARTIFACT_PACKAGE} -> {dependency_name}"),
            true,
        )?;
        let resolved = packages
            .get(string_field(edge, "pkg", "Web artifact resolve edge")?)
            .ok_or_else(|| error("Web artifact resolve package 缺失"))?;
        if package_name(resolved)? != *dependency_name {
            return Err(error(format!(
                "Web artifact resolve edge identity 错误: {dependency_name}"
            )));
        }
    }
    for dependency_name in WEB_ARTIFACT_DEV_DEPENDENCIES {
        let edge = direct_edge(
            node,
            &dependency_name.replace('-', "_"),
            WEB_ARTIFACT_PACKAGE,
        )?;
        let context = format!("{WEB_ARTIFACT_PACKAGE} dev -> {dependency_name}");
        dev_edge(edge, &context)?;
        let resolved = packages
            .get(string_field(edge, "pkg", "Web artifact dev resolve edge")?)
            .ok_or_else(|| error("Web artifact dev resolve package 缺失"))?;
        if package_name(resolved)? != *dependency_name {
            return Err(error(format!(
                "Web artifact dev resolve edge identity 错误: {dependency_name}"
            )));
        }
        let resolved_node = nodes
            .get(string_field(edge, "pkg", "Web artifact dev resolve edge")?)
            .ok_or_else(|| error("Web artifact dev resolve node 缺失"))?;
        let features = unique_string_array(
            resolved_node
                .get("features")
                .ok_or_else(|| error(format!("{context} resolved features 缺失")))?,
            &format!("{context} resolved features"),
        )?;
        if !features.contains(&"default") {
            return Err(error(format!("{context} resolved default feature 缺失")));
        }
    }
    Ok(())
}

fn validate_exact_direct_dependencies(
    package: &Map<String, Value>,
    package_name_expected: &str,
    expected: &[&str],
) -> PolicyResult<()> {
    let actual = package_dependencies(package)?
        .into_iter()
        .map(dependency_identity)
        .collect::<PolicyResult<Vec<_>>>()?;
    let expected = expected.iter().copied().collect::<HashSet<_>>();
    let actual_set = actual.iter().copied().collect::<HashSet<_>>();
    if actual.len() != expected.len() || actual_set != expected {
        return Err(error(format!(
            "{package_name_expected} direct dependencies 必须精确为 {expected:?}，实际 {actual:?}"
        )));
    }
    Ok(())
}

fn validate_exact_normal_dependencies(
    package: &Map<String, Value>,
    package_name_expected: &str,
    expected: &[&str],
) -> PolicyResult<()> {
    let actual = package_dependencies(package)?
        .into_iter()
        .filter(|dependency| normal_kind(dependency.get("kind").and_then(Value::as_str)))
        .map(dependency_identity)
        .collect::<PolicyResult<Vec<_>>>()?;
    let expected = expected.iter().copied().collect::<HashSet<_>>();
    let actual_set = actual.iter().copied().collect::<HashSet<_>>();
    if actual.len() != expected.len() || actual_set != expected {
        return Err(error(format!(
            "{package_name_expected} normal dependencies 必须精确为 {expected:?}，实际 {actual:?}"
        )));
    }
    Ok(())
}

fn validate_exact_dev_dependencies(
    package: &Map<String, Value>,
    package_name_expected: &str,
    expected: &[&str],
) -> PolicyResult<()> {
    let actual = package_dependencies(package)?
        .into_iter()
        .filter(|dependency| dependency.get("kind").and_then(Value::as_str) == Some("dev"))
        .map(dependency_identity)
        .collect::<PolicyResult<Vec<_>>>()?;
    let expected = expected.iter().copied().collect::<HashSet<_>>();
    let actual_set = actual.iter().copied().collect::<HashSet<_>>();
    if actual.len() != expected.len() || actual_set != expected {
        return Err(error(format!(
            "{package_name_expected} dev dependencies 必须精确为 {expected:?}，实际 {actual:?}"
        )));
    }
    Ok(())
}

fn check_registry_declaration(
    dependency: &Map<String, Value>,
    requirement: &str,
    uses_default_features: bool,
    features: &[&str],
    context: &str,
) -> PolicyResult<()> {
    if optional_string_field(dependency, "source", context)? != Some(CRATES_IO_SOURCE)
        || dependency.get("req").and_then(Value::as_str) != Some(requirement)
        || bool_field(dependency, "uses_default_features", context)? != uses_default_features
        || feature_set(dependency, context)? != features.iter().copied().collect::<HashSet<_>>()
    {
        return Err(error(format!("{context} registry declaration 漂移")));
    }
    Ok(())
}

fn load_metadata(root: &Path) -> ToolResult<Value> {
    let lockfile = root.join("Cargo.lock");
    let lock_is_regular = fs::symlink_metadata(&lockfile)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false);
    if !lock_is_regular {
        return Err(error(format!(
            "Cargo.lock 不存在或不是普通文件: {}",
            lockfile.display()
        ))
        .into());
    }
    let command = root.join("scripts/cargo-build-lock.sh");
    let output = Command::new(&command)
        .args([
            "--",
            "cargo",
            "metadata",
            "--locked",
            "--format-version",
            "1",
        ])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(error(format!(
            "cargo metadata 失败（{}）: {}",
            status_description(output.status),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
        .into());
    }
    serde_json::from_slice(&output.stdout).map_err(|json_error| {
        Box::new(error(format!("cargo metadata JSON 解析失败: {json_error}"))) as _
    })
}

pub(crate) fn run(root: &Path) -> ToolResult<()> {
    let metadata = load_metadata(root)?;
    audit_metadata(&metadata)?;
    println!("ok: dependency owner、single-host、schema tooling isolation 与 legacy policy 已通过");
    Ok(())
}

fn audit_metadata(metadata: &Value) -> PolicyResult<()> {
    let (packages, default_members) = package_maps(metadata)?;
    let nodes = resolve_maps(metadata, &packages)?;
    let workspace = workspace_by_name(metadata, &packages)?;
    validate_owner_policies(&workspace, &packages, &nodes)?;
    validate_single_host(&workspace, &packages, &nodes)?;
    validate_runtime_isolation(metadata, &workspace, &default_members, &packages, &nodes)?;
    validate_legacy_graph(&workspace, &packages, &nodes)?;
    validate_tool_and_contract(&workspace, &packages, &nodes)?;
    validate_web_artifact_dependencies(&workspace, &packages, &nodes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{env, fs, time::SystemTime};

    fn id(name: &str) -> String {
        format!("path+file:///workspace/{name}#2.1.3")
    }

    fn registry_id(name: &str, version: &str) -> String {
        format!("{CRATES_IO_SOURCE}#{name}@{version}")
    }

    fn registry_version(name: &str) -> &'static str {
        match name {
            "turso" => "0.7.2",
            "axum" => "0.7.9",
            "ureq" => "2.12.1",
            "rmcp" => "3.1.0",
            "tauri" => "2.11.2",
            "jsonschema" => "0.47.0",
            "schemars" => "1.2.1",
            "serde" => "1.0.228",
            "serde_json" => "1.0.150",
            "sha2" => "0.10.9",
            "libc" => "0.2.186",
            "fs4" => "0.13.1",
            "tempfile" => "3.27.0",
            _ => "1.0.0",
        }
    }

    fn dependency(
        name: &str,
        source: Option<&str>,
        req: &str,
        default: bool,
        features: &[&str],
    ) -> Value {
        json!({
            "name": name,
            "source": source,
            "req": req,
            "kind": null,
            "rename": null,
            "optional": false,
            "uses_default_features": default,
            "features": features,
            "target": null,
            "registry": null,
        })
    }

    fn local_dependency(name: &str, features: &[&str]) -> Value {
        let mut dependency = dependency(name, None, "*", false, features);
        dependency["path"] = json!(format!("/workspace/{name}"));
        dependency
    }

    fn registry_dependency(name: &str, req: &str, default: bool, features: &[&str]) -> Value {
        dependency(name, Some(CRATES_IO_SOURCE), req, default, features)
    }

    fn dev_registry_dependency(name: &str, req: &str, default: bool, features: &[&str]) -> Value {
        let mut dependency = registry_dependency(name, req, default, features);
        dependency["kind"] = json!("dev");
        dependency
    }

    fn package(name: &str, dependencies: Vec<Value>) -> Value {
        json!({
            "id": id(name),
            "name": name,
            "version": "2.1.3",
            "source": null,
            "manifest_path": format!("/workspace/{name}/Cargo.toml"),
            "dependencies": dependencies,
            "targets": [],
            "features": if name == CONTRACT_PACKAGE { json!({"default": [], "schema": ["dep:schemars"]}) } else { json!({}) },
        })
    }

    fn registry_package(name: &str) -> Value {
        let version = registry_version(name);
        json!({
            "id": registry_id(name, version),
            "name": name,
            "version": version,
            "source": CRATES_IO_SOURCE,
            "manifest_path": format!("/cargo/registry/{name}-{version}/Cargo.toml"),
            "dependencies": [],
            "targets": [],
            "features": {},
        })
    }

    fn edge(name: &str, package_id: &str) -> Value {
        json!({
            "name": name.replace('-', "_"),
            "pkg": package_id,
            "dep_kinds": [{"kind": null, "target": null}],
        })
    }

    fn dev_edge(name: &str, package_id: &str) -> Value {
        json!({
            "name": name.replace('-', "_"),
            "pkg": package_id,
            "dep_kinds": [{"kind": "dev", "target": null}],
        })
    }

    fn node(package_id: &str, edges: Vec<Value>, features: &[&str]) -> Value {
        let dependencies = edges
            .iter()
            .map(|edge| edge["pkg"].clone())
            .collect::<Vec<_>>();
        json!({
            "id": package_id,
            "dependencies": dependencies,
            "deps": edges,
            "features": features,
        })
    }

    #[allow(clippy::vec_init_then_push)]
    fn fixture() -> Value {
        let workspace_names = [
            "kanban-core",
            SERVICE_PACKAGE,
            CONTRACT_PACKAGE,
            WEB_ARTIFACT_PACKAGE,
            "kanban-client",
            "kanban-cli",
            "kanban-mcp",
            SERVER_PACKAGE,
            "kanban-desktop",
            TOOL_PACKAGE,
        ];
        let mut packages = Vec::new();
        packages.push(package("kanban-core", vec![]));
        packages.push(package(
            SERVICE_PACKAGE,
            vec![registry_dependency("turso", "=0.7.2", false, &["fts"])],
        ));
        packages.push(package(
            CONTRACT_PACKAGE,
            vec![
                {
                    let mut dependency =
                        registry_dependency("schemars", "^1.2.1", false, &["std", "derive"]);
                    dependency["optional"] = json!(true);
                    dependency
                },
                registry_dependency("serde", "^1.0", true, &["derive"]),
                registry_dependency("serde_json", "^1.0", true, &[]),
                registry_dependency("sha2", "^0.10", true, &[]),
            ],
        ));
        packages.push(package(
            WEB_ARTIFACT_PACKAGE,
            vec![
                local_dependency(CONTRACT_PACKAGE, &[]),
                registry_dependency("libc", "^0.2", false, &[]),
                dev_registry_dependency("serde_json", "^1.0", true, &[]),
                dev_registry_dependency("tempfile", "^3.10", true, &[]),
            ],
        ));
        packages.push(package(
            "kanban-client",
            vec![registry_dependency("ureq", "^2.12", false, &["json"])],
        ));
        packages.push(package("kanban-cli", vec![]));
        packages.push(package(
            "kanban-mcp",
            vec![
                local_dependency(CONTRACT_PACKAGE, &["schema"]),
                registry_dependency(
                    "rmcp",
                    "=3.1.0",
                    false,
                    &["macros", "server", "transport-io"],
                ),
            ],
        ));
        packages.push(package(
            SERVER_PACKAGE,
            vec![
                local_dependency(SERVICE_PACKAGE, &[]),
                registry_dependency("axum", "^0.7", true, &[]),
            ],
        ));
        packages.push(package(
            "kanban-desktop",
            vec![registry_dependency("tauri", "^2", true, &["tray-icon"])],
        ));
        packages.push(package(
            TOOL_PACKAGE,
            vec![
                registry_dependency(FS4_PACKAGE, "^0.13.1", true, &[]),
                registry_dependency(JSONSCHEMA_PACKAGE, "^0.47.0", false, &[]),
                local_dependency(CONTRACT_PACKAGE, &["schema"]),
                local_dependency(WEB_ARTIFACT_PACKAGE, &[]),
                registry_dependency("serde", "^1.0", true, &["derive"]),
                registry_dependency("serde_json", "^1.0", true, &[]),
                registry_dependency("sha2", "^0.10", true, &[]),
            ],
        ));
        for name in [
            FS4_PACKAGE,
            "turso",
            "axum",
            "ureq",
            "rmcp",
            "tauri",
            "libc",
            JSONSCHEMA_PACKAGE,
            SCHEMARS_PACKAGE,
            "serde",
            "serde_json",
            "sha2",
            "tempfile",
        ] {
            packages.push(registry_package(name));
        }

        let mut nodes = Vec::new();
        nodes.push(node(&id("kanban-core"), vec![], &[]));
        nodes.push(node(
            &id(SERVICE_PACKAGE),
            vec![edge("turso", &registry_id("turso", "0.7.2"))],
            &[],
        ));
        nodes.push(node(
            &id(CONTRACT_PACKAGE),
            vec![
                edge(SCHEMARS_PACKAGE, &registry_id(SCHEMARS_PACKAGE, "1.2.1")),
                edge("serde", &registry_id("serde", "1.0.228")),
                edge("serde_json", &registry_id("serde_json", "1.0.150")),
                edge("sha2", &registry_id("sha2", "0.10.9")),
            ],
            &["default", "schema"],
        ));
        nodes.push(node(
            &id(WEB_ARTIFACT_PACKAGE),
            vec![
                edge(CONTRACT_PACKAGE, &id(CONTRACT_PACKAGE)),
                edge("libc", &registry_id("libc", "0.2.186")),
                dev_edge("serde_json", &registry_id("serde_json", "1.0.150")),
                dev_edge("tempfile", &registry_id("tempfile", "3.27.0")),
            ],
            &[],
        ));
        nodes.push(node(
            &id("kanban-client"),
            vec![edge("ureq", &registry_id("ureq", "2.12.1"))],
            &[],
        ));
        nodes.push(node(&id("kanban-cli"), vec![], &[]));
        nodes.push(node(
            &id("kanban-mcp"),
            vec![
                edge(CONTRACT_PACKAGE, &id(CONTRACT_PACKAGE)),
                edge("rmcp", &registry_id("rmcp", "3.1.0")),
            ],
            &[],
        ));
        nodes.push(node(
            &id(SERVER_PACKAGE),
            vec![
                edge(SERVICE_PACKAGE, &id(SERVICE_PACKAGE)),
                edge("axum", &registry_id("axum", "0.7.9")),
            ],
            &[],
        ));
        nodes.push(node(
            &id("kanban-desktop"),
            vec![edge("tauri", &registry_id("tauri", "2.11.2"))],
            &[],
        ));
        nodes.push(node(
            &id(TOOL_PACKAGE),
            vec![
                edge(FS4_PACKAGE, &registry_id(FS4_PACKAGE, "0.13.1")),
                edge(
                    JSONSCHEMA_PACKAGE,
                    &registry_id(JSONSCHEMA_PACKAGE, "0.47.0"),
                ),
                edge(CONTRACT_PACKAGE, &id(CONTRACT_PACKAGE)),
                edge(WEB_ARTIFACT_PACKAGE, &id(WEB_ARTIFACT_PACKAGE)),
                edge("serde", &registry_id("serde", "1.0.228")),
                edge("serde_json", &registry_id("serde_json", "1.0.150")),
                edge("sha2", &registry_id("sha2", "0.10.9")),
            ],
            &[],
        ));
        for name in [
            FS4_PACKAGE,
            "turso",
            "axum",
            "ureq",
            "rmcp",
            "tauri",
            "libc",
            JSONSCHEMA_PACKAGE,
            "serde",
            "serde_json",
            "sha2",
            "tempfile",
        ] {
            let features = match name {
                "turso" => vec!["fts"],
                "ureq" => vec!["json"],
                "rmcp" => vec!["macros", "server", "transport-io"],
                "tauri" => vec!["default", "tray-icon"],
                "axum" => vec!["default"],
                "libc" => vec!["default"],
                "fs4" => vec!["default"],
                "tempfile" => vec!["default"],
                "serde_json" => vec!["default"],
                _ => vec![],
            };
            nodes.push(node(
                &registry_id(name, registry_version(name)),
                vec![],
                &features,
            ));
        }
        nodes.push(node(
            &registry_id(SCHEMARS_PACKAGE, "1.2.1"),
            vec![],
            &["chrono04", "default", "derive", "schemars_derive", "std"],
        ));

        let workspace_members = workspace_names
            .iter()
            .map(|name| id(name))
            .collect::<Vec<_>>();
        let default_members = workspace_names[..8]
            .iter()
            .map(|name| id(name))
            .collect::<Vec<_>>();
        json!({
            "packages": packages,
            "workspace_members": workspace_members,
            "workspace_default_members": default_members,
            "resolve": {"root": null, "nodes": nodes},
        })
    }

    fn assert_reject(mut metadata: Value, mutate: impl FnOnce(&mut Value)) {
        mutate(&mut metadata);
        assert!(audit_metadata(&metadata).is_err());
    }

    fn package_record<'a>(metadata: &'a mut Value, name: &str) -> &'a mut Map<String, Value> {
        metadata["packages"]
            .as_array_mut()
            .expect("packages")
            .iter_mut()
            .find(|package| package["name"] == name)
            .and_then(Value::as_object_mut)
            .expect("package record")
    }

    fn dependency_record<'a>(
        metadata: &'a mut Value,
        owner: &str,
        dependency: &str,
    ) -> &'a mut Map<String, Value> {
        package_record(metadata, owner)["dependencies"]
            .as_array_mut()
            .expect("dependencies")
            .iter_mut()
            .find(|item| item["name"] == dependency)
            .and_then(Value::as_object_mut)
            .expect("dependency record")
    }

    fn node_record<'a>(metadata: &'a mut Value, name: &str) -> &'a mut Map<String, Value> {
        let package_id = id(name);
        metadata["resolve"]["nodes"]
            .as_array_mut()
            .expect("nodes")
            .iter_mut()
            .find(|node| node["id"] == package_id)
            .and_then(Value::as_object_mut)
            .expect("node record")
    }

    #[test]
    fn clean_fixture_passes() {
        audit_metadata(&fixture()).expect("clean dependency fixture should pass");
    }

    #[test]
    fn web_artifact_and_protocol_dependency_boundaries_are_frozen() {
        assert_reject(fixture(), |metadata| {
            package_record(metadata, WEB_ARTIFACT_PACKAGE)["dependencies"]
                .as_array_mut()
                .unwrap()
                .push(registry_dependency("serde", "^1.0", true, &["derive"]));
        });
        assert_reject(fixture(), |metadata| {
            dependency_record(metadata, CONTRACT_PACKAGE, "sha2")["req"] = json!("^0.11");
        });
        assert_reject(fixture(), |metadata| {
            dependency_record(metadata, WEB_ARTIFACT_PACKAGE, "libc")["uses_default_features"] =
                json!(true);
        });
        assert_reject(fixture(), |metadata| {
            let mut dependency = registry_dependency("sha2", "^0.10", true, &[]);
            dependency["kind"] = json!("dev");
            package_record(metadata, WEB_ARTIFACT_PACKAGE)["dependencies"]
                .as_array_mut()
                .unwrap()
                .push(dependency);
        });
        assert_reject(fixture(), |metadata| {
            dependency_record(metadata, WEB_ARTIFACT_PACKAGE, "tempfile")["target"] =
                json!("cfg(unix)");
        });
        assert_reject(fixture(), |metadata| {
            let mut dependency = registry_dependency("sha2", "^0.10", true, &[]);
            dependency["kind"] = json!("build");
            package_record(metadata, WEB_ARTIFACT_PACKAGE)["dependencies"]
                .as_array_mut()
                .unwrap()
                .push(dependency);
        });
    }

    #[test]
    fn duplicate_owner_is_rejected() {
        assert_reject(fixture(), |metadata| {
            package_record(metadata, SERVER_PACKAGE)["dependencies"]
                .as_array_mut()
                .unwrap()
                .push(registry_dependency("turso", "=0.7.2", false, &["fts"]));
        });
    }

    #[test]
    fn wrong_kind_optional_target_source_version_default_and_feature_are_rejected() {
        for mutation in [
            |metadata: &mut Value| {
                dependency_record(metadata, SERVICE_PACKAGE, "turso")["kind"] = json!("dev")
            },
            |metadata: &mut Value| {
                dependency_record(metadata, SERVICE_PACKAGE, "turso")["optional"] = json!(true)
            },
            |metadata: &mut Value| {
                dependency_record(metadata, SERVICE_PACKAGE, "turso")["target"] = json!("cfg(unix)")
            },
            |metadata: &mut Value| {
                dependency_record(metadata, SERVICE_PACKAGE, "turso")["source"] =
                    json!("git+https://example.invalid/turso")
            },
            |metadata: &mut Value| package_record(metadata, "turso")["version"] = json!("0.7.1"),
            |metadata: &mut Value| {
                dependency_record(metadata, SERVICE_PACKAGE, "turso")["uses_default_features"] =
                    json!(true)
            },
            |metadata: &mut Value| {
                dependency_record(metadata, SERVICE_PACKAGE, "turso")["features"] = json!([])
            },
        ] {
            assert_reject(fixture(), mutation);
        }
    }

    #[test]
    fn alias_and_resolve_identity_are_rejected() {
        assert_reject(fixture(), |metadata| {
            dependency_record(metadata, SERVICE_PACKAGE, "turso")["rename"] = json!("storage");
        });
        assert_reject(fixture(), |metadata| {
            node_record(metadata, SERVICE_PACKAGE)["deps"][0]["name"] = json!("storage");
        });
    }

    #[test]
    fn runtime_tooling_leak_is_rejected() {
        assert_reject(fixture(), |metadata| {
            package_record(metadata, SERVICE_PACKAGE)["dependencies"]
                .as_array_mut()
                .unwrap()
                .push(local_dependency(TOOL_PACKAGE, &[]));
            node_record(metadata, SERVICE_PACKAGE)["deps"]
                .as_array_mut()
                .unwrap()
                .push(edge(TOOL_PACKAGE, &id(TOOL_PACKAGE)));
            node_record(metadata, SERVICE_PACKAGE)["dependencies"]
                .as_array_mut()
                .unwrap()
                .push(json!(id(TOOL_PACKAGE)));
        });
    }

    #[test]
    fn legacy_package_in_active_graph_is_rejected() {
        assert_reject(fixture(), |metadata| {
            metadata["packages"]
                .as_array_mut()
                .unwrap()
                .push(package("kanban-local", vec![]));
            metadata["resolve"]["nodes"]
                .as_array_mut()
                .unwrap()
                .push(node(&id("kanban-local"), vec![], &[]));
            package_record(metadata, SERVICE_PACKAGE)["dependencies"]
                .as_array_mut()
                .unwrap()
                .push(local_dependency("kanban-local", &[]));
            node_record(metadata, SERVICE_PACKAGE)["deps"]
                .as_array_mut()
                .unwrap()
                .push(edge("kanban-local", &id("kanban-local")));
            node_record(metadata, SERVICE_PACKAGE)["dependencies"]
                .as_array_mut()
                .unwrap()
                .push(json!(id("kanban-local")));
        });
    }

    #[test]
    fn single_host_wrong_edge_is_rejected() {
        assert_reject(fixture(), |metadata| {
            node_record(metadata, SERVER_PACKAGE)["deps"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|edge| edge["name"] == "kanban_service")
                .unwrap()["pkg"] = json!(id("kanban-core"));
            node_record(metadata, SERVER_PACKAGE)["dependencies"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|package_id| package_id.as_str() == Some(&id(SERVICE_PACKAGE)))
                .unwrap()
                .clone_from(&json!(id("kanban-core")));
        });
    }

    #[test]
    fn missing_lock_is_rejected_before_metadata_command() {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = env::temp_dir().join(format!("xtask-deps-lock-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).expect("fixture root");
        let result = load_metadata(&root);
        assert!(result.is_err());
        let _ = fs::remove_dir(&root);
    }
}
