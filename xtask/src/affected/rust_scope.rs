//! 用 Cargo 的工作区路径依赖收窄 Rust gate；解析失败时升级为完整 gate。
//!
//! 不按测试名、目录名中的 tests 或耗时猜测覆盖范围；normal/build/dev、optional
//! 和 target-specific 路径依赖都参与反向闭包。跨进程和生成物边界仍由父模块路由。
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
    process::Command,
};

use serde::Deserialize;
use xtask::ToolResult;

use super::{Plan, Recipe, is_rust_product};

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    // metadata 的 name 是原包名；rename 不改变路径 owner。
    name: String,
    path: Option<PathBuf>,
}

pub(super) fn refine(root: &Path, plan: &mut Plan) {
    if !plan.recipes.contains(&Recipe::RustFast) {
        return;
    }
    match select(root, &plan.changed_files) {
        Ok(packages) => {
            plan.rust_packages = packages;
            plan.rust_scope_reason = Some("变更 owner 与全部工作区路径依赖消费者".to_owned());
        }
        Err(error) => {
            // 不以空选择、成功退出或旧的 core 子集掩盖无法证明的覆盖范围。
            for recipe in &mut plan.recipes {
                if *recipe == Recipe::RustFast {
                    *recipe = Recipe::RustFull;
                }
            }
            plan.rust_packages.clear();
            plan.rust_scope_reason = Some(format!("无法安全收窄，使用 rust-full：{error}"));
        }
    }
}

fn select(root: &Path, changed: &[String]) -> ToolResult<Vec<String>> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
            "--locked",
        ])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "cargo metadata 失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
        .into());
    }
    let metadata: Metadata = serde_json::from_slice(&output.stdout)?;
    let root = root.canonicalize()?;
    let members = metadata
        .workspace_members
        .into_iter()
        .collect::<BTreeSet<_>>();
    let packages = metadata
        .packages
        .into_iter()
        .filter(|package| members.contains(&package.id))
        .collect::<Vec<_>>();
    if packages.is_empty() || packages.len() != members.len() {
        return Err(std::io::Error::other("工作区成员清单不完整").into());
    }
    let mut roots = BTreeMap::new();
    let mut names = BTreeSet::new();
    for package in &packages {
        let directory = package
            .manifest_path
            .parent()
            .ok_or_else(|| std::io::Error::other("manifest 没有父目录"))?
            .canonicalize()?;
        if roots.insert(directory, package.name.clone()).is_some()
            || !names.insert(package.name.clone())
        {
            return Err(std::io::Error::other("工作区 owner 不唯一").into());
        }
    }
    let mut selected = BTreeSet::new();
    for path in changed.iter().filter(|path| is_rust_product(path)) {
        let relative = Path::new(path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(std::io::Error::other(format!("不支持的变更路径：{path}")).into());
        }
        // 删除的源码仍要找到 owner，因此不能 canonicalize 变更文件本身。
        let absolute = root.join(relative);
        let owner = roots
            .iter()
            .filter(|(directory, _)| absolute.starts_with(directory))
            .max_by_key(|(directory, _)| directory.components().count())
            .map(|(_, name)| name)
            .ok_or_else(|| std::io::Error::other(format!("无法确定 owner：{path}")))?;
        selected.insert(owner.clone());
    }
    if selected.is_empty() {
        return Err(std::io::Error::other("Rust gate 没有可证明的变更 owner").into());
    }
    let mut consumers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for package in &packages {
        for dependency in &package.dependencies {
            if let Some(path) = &dependency.path {
                let directory = path.canonicalize()?;
                if let Some(owner) = roots.get(&directory) {
                    consumers
                        .entry(owner.clone())
                        .or_default()
                        .insert(package.name.clone());
                }
            } else if names.contains(&dependency.name) {
                // 保守包含同名 workspace member，覆盖 registry 依赖被 patch 回工作区
                // 的可能性；不声称 --no-deps 给出了当前 feature 的精确 resolve 图。
                consumers
                    .entry(dependency.name.clone())
                    .or_default()
                    .insert(package.name.clone());
            }
        }
    }
    Ok(reverse_closure(selected, &consumers).into_iter().collect())
}

fn reverse_closure(
    mut selected: BTreeSet<String>,
    consumers: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    let mut pending = selected.iter().cloned().collect::<Vec<_>>();
    while let Some(owner) = pending.pop() {
        if let Some(dependents) = consumers.get(&owner) {
            for dependent in dependents {
                if selected.insert(dependent.clone()) {
                    pending.push(dependent.clone());
                }
            }
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_closure_preserves_owners_consumers_and_terminates_on_cycles() {
        let edges = BTreeMap::from([
            ("core".into(), BTreeSet::from(["service".into()])),
            (
                "service".into(),
                BTreeSet::from(["server".into(), "xtask".into()]),
            ),
            // dev-dependency 可以形成工作区消费者环；每个包仍只加入一次。
            (
                "server".into(),
                BTreeSet::from(["service".into(), "cli".into()]),
            ),
        ]);
        for (owners, expected) in [
            (vec!["cli"], vec!["cli"]),
            (vec!["service"], vec!["cli", "server", "service", "xtask"]),
            (
                vec!["core"],
                vec!["cli", "core", "server", "service", "xtask"],
            ),
            (vec!["isolated", "cli"], vec!["cli", "isolated"]),
        ] {
            let input = owners.into_iter().map(str::to_owned).collect();
            let expected = expected
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>();
            assert_eq!(reverse_closure(input, &edges), expected);
        }
    }
}
