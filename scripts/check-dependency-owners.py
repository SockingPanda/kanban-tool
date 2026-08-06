#!/usr/bin/env python3
"""用 Cargo metadata 锁定 active product dependency 的 owner 与 feature 边界。"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"

# 这些 dependency 属于 active product 的专用边界。root 只声明 identity，
# 具体 feature 必须由唯一实际使用者在 leaf manifest 中选择。
DEPENDENCY_POLICIES: dict[str, dict[str, Any]] = {
    "turso": {
        "owner": "kanban-service",
        "root": {"version": "=0.7.2", "default-features": False},
        "features": {"uses_default_features": False, "features": {"fts"}},
    },
    "axum": {
        "owner": "kanban-server",
        "root": "0.7",
        "features": {"uses_default_features": True, "features": set()},
    },
    "ureq": {
        "owner": "kanban-client",
        "root": {"version": "2.12", "default-features": False},
        "features": {"uses_default_features": False, "features": {"json"}},
    },
    "rmcp": {
        "owner": "kanban-mcp",
        "root": {"version": "=3.1.0", "default-features": False},
        "features": {
            "uses_default_features": False,
            "features": {"macros", "server", "transport-io"},
        },
    },
    "tauri": {
        "owner": "kanban-desktop",
        "root": "2",
        "features": {"uses_default_features": True, "features": {"tray-icon"}},
    },
}


class DependencyOwnerPolicyError(RuntimeError):
    """依赖 owner、resolved identity 或 leaf feature 偏离 canonical policy。"""


def _package_records(metadata: dict[str, Any]) -> dict[str, dict[str, Any]]:
    packages = metadata.get("packages")
    if not isinstance(packages, list) or not all(
        isinstance(package, dict) for package in packages
    ):
        raise DependencyOwnerPolicyError("cargo metadata 缺少有效 packages")
    records: dict[str, dict[str, Any]] = {}
    for package in packages:
        package_id = package.get("id")
        if not isinstance(package_id, str) or not package_id:
            raise DependencyOwnerPolicyError(f"cargo metadata package id 无效: {package}")
        if package_id in records:
            raise DependencyOwnerPolicyError(f"cargo metadata package id 重复: {package_id}")
        records[package_id] = package
    return records


def _workspace_packages(metadata: dict[str, Any]) -> list[dict[str, Any]]:
    members = metadata.get("workspace_members")
    if not isinstance(members, list) or not all(
        isinstance(member, str) and member for member in members
    ):
        raise DependencyOwnerPolicyError("cargo metadata 缺少有效 workspace_members")
    records = _package_records(metadata)
    missing = sorted(set(members) - records.keys())
    if missing:
        raise DependencyOwnerPolicyError(f"cargo metadata 缺少 workspace package: {missing}")
    return [records[member] for member in members]


def _package_name(package: dict[str, Any]) -> str:
    name = package.get("name")
    if not isinstance(name, str) or not name:
        raise DependencyOwnerPolicyError(f"cargo metadata package 缺少 name: {package}")
    return name


def _dependencies(package: dict[str, Any]) -> list[dict[str, Any]]:
    dependencies = package.get("dependencies")
    if not isinstance(dependencies, list) or not all(
        isinstance(dependency, dict) for dependency in dependencies
    ):
        raise DependencyOwnerPolicyError(
            f"{_package_name(package)} cargo metadata dependencies 格式无效"
        )
    return dependencies


def _dependency_matches(dependency: dict[str, Any], package_name: str) -> bool:
    # metadata 的 name 是 package identity；少数 Cargo 输出会额外提供
    # package 字段表示 alias 的真实 package，不能把 rename alias 本身当 owner。
    identity = dependency.get("package") or dependency.get("name")
    return identity == package_name


def _resolved_nodes(metadata: dict[str, Any]) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]]]:
    records = _package_records(metadata)
    resolve = metadata.get("resolve")
    if not isinstance(resolve, dict):
        raise DependencyOwnerPolicyError("cargo metadata 缺少 resolve graph")
    nodes = resolve.get("nodes")
    if not isinstance(nodes, list) or not all(isinstance(node, dict) for node in nodes):
        raise DependencyOwnerPolicyError("cargo metadata resolve.nodes 格式无效")
    by_id: dict[str, dict[str, Any]] = {}
    for node in nodes:
        package_id = node.get("id")
        if not isinstance(package_id, str) or not package_id:
            raise DependencyOwnerPolicyError(f"resolve node 缺少 id: {node}")
        if package_id in by_id:
            raise DependencyOwnerPolicyError(f"resolve node id 重复: {package_id}")
        by_id[package_id] = node
    return records, by_id


def _resolved_direct_package(
    owner: dict[str, Any],
    dependency_name: str,
    records: dict[str, dict[str, Any]],
    nodes: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    owner_id = owner.get("id")
    if not isinstance(owner_id, str):
        raise DependencyOwnerPolicyError(f"{_package_name(owner)} package id 无效")
    node = nodes.get(owner_id)
    if node is None:
        raise DependencyOwnerPolicyError(
            f"{_package_name(owner)} 缺少 resolve node"
        )
    edges = node.get("deps")
    if not isinstance(edges, list) or not all(isinstance(edge, dict) for edge in edges):
        raise DependencyOwnerPolicyError(
            f"{_package_name(owner)} resolve deps 格式无效"
        )
    matches: list[dict[str, Any]] = []
    for edge in edges:
        package_id = edge.get("pkg")
        if not isinstance(package_id, str):
            raise DependencyOwnerPolicyError(
                f"{_package_name(owner)} resolve edge 缺少 pkg: {edge}"
            )
        record = records.get(package_id)
        if record is None:
            raise DependencyOwnerPolicyError(
                f"{_package_name(owner)} resolve edge 指向未知 package: {package_id}"
            )
        if record.get("name") == dependency_name or edge.get("name") == dependency_name:
            matches.append(record)
            dep_kinds = edge.get("dep_kinds")
            if dep_kinds != [{"kind": None, "target": None}]:
                raise DependencyOwnerPolicyError(
                    f"{_package_name(owner)} -> {dependency_name} 必须是唯一 unconditional normal edge"
                )
    if len(matches) != 1:
        raise DependencyOwnerPolicyError(
            f"{_package_name(owner)} resolved {dependency_name} edge 数量必须为 1，实际 {len(matches)}"
        )
    return matches[0]


def _root_declaration(root: Path, dependency_name: str) -> Any:
    try:
        with (root / "Cargo.toml").open("rb") as handle:
            manifest = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise DependencyOwnerPolicyError(f"读取 root Cargo.toml 失败: {error}") from error
    workspace = manifest.get("workspace")
    if not isinstance(workspace, dict):
        raise DependencyOwnerPolicyError("root Cargo.toml 缺少 [workspace]")
    dependencies = workspace.get("dependencies")
    if not isinstance(dependencies, dict):
        raise DependencyOwnerPolicyError("root Cargo.toml 缺少 [workspace.dependencies]")
    if dependency_name not in dependencies:
        raise DependencyOwnerPolicyError(
            f"root workspace dependency 缺少 {dependency_name}"
        )
    return dependencies[dependency_name]


def _check_root_declaration(root: Path, dependency_name: str, expected: Any) -> Any:
    actual = _root_declaration(root, dependency_name)
    if actual != expected:
        raise DependencyOwnerPolicyError(
            f"workspace dependency {dependency_name} identity 漂移: expected={expected!r}, actual={actual!r}"
        )
    return actual


def _normalized_req(root_requirement: Any) -> str:
    if isinstance(root_requirement, str):
        return root_requirement if root_requirement.startswith("=") else f"^{root_requirement}"
    version = root_requirement.get("version") if isinstance(root_requirement, dict) else None
    if not isinstance(version, str) or not version:
        raise DependencyOwnerPolicyError(f"workspace dependency version 无效: {root_requirement!r}")
    return version if version.startswith("=") else f"^{version}"


def _version_satisfies(version: object, requirement: str) -> bool:
    if not isinstance(version, str) or not version:
        return False
    if requirement.startswith("="):
        return version == requirement[1:]
    expected = requirement.removeprefix("^").split(".")
    actual = version.split(".")
    return len(actual) >= len(expected) and actual[: len(expected)] == expected


def _check_leaf_features(
    owner: dict[str, Any], dependency_name: str, dependency: dict[str, Any], expected: dict[str, Any]
) -> None:
    actual_features = dependency.get("features")
    if not isinstance(actual_features, list) or not all(
        isinstance(feature, str) for feature in actual_features
    ):
        raise DependencyOwnerPolicyError(
            f"{_package_name(owner)} -> {dependency_name} features 格式无效"
        )
    if set(actual_features) != expected["features"]:
        raise DependencyOwnerPolicyError(
            f"{_package_name(owner)} -> {dependency_name} leaf features 漂移: "
            f"expected={sorted(expected['features'])}, actual={sorted(actual_features)}"
        )
    if dependency.get("uses_default_features") is not expected["uses_default_features"]:
        raise DependencyOwnerPolicyError(
            f"{_package_name(owner)} -> {dependency_name} default feature policy 漂移: "
            f"expected={expected['uses_default_features']}, actual={dependency.get('uses_default_features')}"
        )
    if dependency.get("optional") is not False:
        raise DependencyOwnerPolicyError(
            f"{_package_name(owner)} -> {dependency_name} 必须是非 optional product dependency"
        )
    if dependency.get("kind") != "normal":
        raise DependencyOwnerPolicyError(
            f"{_package_name(owner)} -> {dependency_name} 必须是 normal dependency"
        )
    if dependency.get("source") != CRATES_IO_SOURCE or dependency.get("registry") is not None:
        raise DependencyOwnerPolicyError(
            f"{_package_name(owner)} -> {dependency_name} manifest source 必须来自 crates.io"
        )
    if dependency.get("target") is not None:
        raise DependencyOwnerPolicyError(
            f"{_package_name(owner)} -> {dependency_name} 不得通过 target-specific dependency 隐藏 owner"
        )


def audit_metadata(metadata: dict[str, Any], root: Path = ROOT) -> None:
    """校验五个专用 dependency 的唯一 owner、root identity 与 leaf feature。"""

    workspace = _workspace_packages(metadata)
    records, nodes = _resolved_nodes(metadata)
    for dependency_name, policy in DEPENDENCY_POLICIES.items():
        _check_root_declaration(root, dependency_name, policy["root"])
        owners: list[tuple[dict[str, Any], dict[str, Any]]] = []
        for package in workspace:
            matches = [
                dependency
                for dependency in _dependencies(package)
                if _dependency_matches(dependency, dependency_name)
            ]
            if len(matches) > 1:
                raise DependencyOwnerPolicyError(
                    f"{_package_name(package)} 直接声明 {dependency_name} 超过一次"
                )
            if matches:
                owners.append((package, matches[0]))
        expected_owner = policy["owner"]
        actual_owners = [_package_name(package) for package, _ in owners]
        if actual_owners != [expected_owner]:
            raise DependencyOwnerPolicyError(
                f"{dependency_name} 必须只有 owner {expected_owner}，实际 owners={actual_owners}"
            )
        owner, dependency = owners[0]
        _check_leaf_features(owner, dependency_name, dependency, policy["features"])

        resolved = _resolved_direct_package(owner, dependency_name, records, nodes)
        if resolved.get("source") != CRATES_IO_SOURCE:
            raise DependencyOwnerPolicyError(
                f"{_package_name(owner)} -> {dependency_name} source 必须是 crates.io: "
                f"{resolved.get('source')}"
            )
        expected_root = policy["root"]
        expected_req = _normalized_req(expected_root)
        if resolved.get("version") is None or resolved.get("name") != dependency_name:
            raise DependencyOwnerPolicyError(
                f"{_package_name(owner)} resolved package identity 错误: {resolved}"
            )
        if not _version_satisfies(resolved.get("version"), expected_req):
            raise DependencyOwnerPolicyError(
                f"{_package_name(owner)} -> {dependency_name} resolved version 不满足 {expected_req}: "
                f"{resolved.get('version')}"
            )
        matching_dependencies = [
            candidate
            for candidate in _dependencies(owner)
            if _dependency_matches(candidate, dependency_name)
        ]
        if matching_dependencies[0].get("req") != expected_req:
            raise DependencyOwnerPolicyError(
                f"{_package_name(owner)} -> {dependency_name} resolved req 漂移: "
                f"expected={expected_req!r}, actual={matching_dependencies[0].get('req')!r}"
            )


def _inherited_lock_pass_fds() -> tuple[int, ...]:
    raw = os.environ.get("KANBAN_CARGO_BUILD_LOCK_FD", "")
    if not (
        os.environ.get("KANBAN_CARGO_BUILD_LOCK_HELD") == "1"
        and raw.isascii()
        and raw.isdecimal()
        and raw[0] in "3456789"
    ):
        return ()
    try:
        fd = int(raw)
        os.fstat(fd)
    except (OSError, OverflowError, ValueError):
        return ()
    return (fd,)


def load_metadata(root: Path = ROOT) -> dict[str, Any]:
    command = [
        str(root / "scripts/cargo-build-lock.sh"),
        "--",
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--locked",
        "--manifest-path",
        "Cargo.toml",
    ]
    completed = subprocess.run(
        command,
        cwd=root,
        check=False,
        close_fds=True,
        pass_fds=_inherited_lock_pass_fds(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise DependencyOwnerPolicyError(
            f"cargo metadata 失败 ({completed.returncode}): {completed.stderr.strip()}"
        )
    try:
        metadata = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise DependencyOwnerPolicyError(f"cargo metadata JSON 无效: {error}") from error
    if not isinstance(metadata, dict):
        raise DependencyOwnerPolicyError("cargo metadata 顶层必须是 object")
    return metadata


def main() -> int:
    try:
        audit_metadata(load_metadata())
    except DependencyOwnerPolicyError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print("ok: turso/axum/ureq/rmcp/tauri 的唯一 owner、resolved source/version 与 leaf feature policy 已锁定")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
