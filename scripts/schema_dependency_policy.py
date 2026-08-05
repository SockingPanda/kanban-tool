#!/usr/bin/env python3
"""校验 Phase 1 schema tooling 的声明、resolved identity 与产品隔离。"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tomllib
from collections import Counter
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PACKAGE = "kanban-contract"
TOOL_PACKAGE = "xtask"
TOOL_MEMBER_PATH = "xtask"
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
APPROVED_REGISTRY_CLOSURE_PATH = (
    "policy/schema-tool-registry-closure.json"
)
REGISTRY_CLOSURE_FORMAT_VERSION = 1
LOCKFILE_VERSION = 4
TOOL_DIRECT_DEPENDENCY_NAMES = (
    "jsonschema",
    CONTRACT_PACKAGE,
    "serde",
    "serde_json",
    "sha2",
)
CONTRACT_DIRECT_DEPENDENCY_NAMES = ("schemars", "serde", "serde_json")
TOOL_MANIFEST_DEPENDENCIES = {
    "jsonschema": {"workspace": True},
    CONTRACT_PACKAGE: {
        "workspace": True,
        "default-features": False,
        "features": ["schema"],
    },
    "serde": {"workspace": True},
    "serde_json": {"workspace": True},
    "sha2": {"workspace": True},
}
CONTRACT_MANIFEST_FEATURES = {
    "default": [],
    "schema": ["dep:schemars"],
}
CONTRACT_MANIFEST_DEPENDENCIES = {
    "schemars": {"workspace": True, "optional": True},
    "serde": {"workspace": True},
    "serde_json": {"workspace": True},
}
WORKSPACE_CANONICAL_DEPENDENCIES = {
    "jsonschema": {"version": "0.47.0", "default-features": False},
    CONTRACT_PACKAGE: {
        "path": "crates/kanban-contract",
        "default-features": False,
    },
    "serde": {"version": "1.0", "features": ["derive"]},
    "schemars": {
        "version": "1.2.1",
        "default-features": False,
        "features": ["std", "derive"],
    },
    "serde_json": "1.0",
    "sha2": "0.10",
}
CORE_PACKAGES = (
    "kanban-core",
    "kanban-application",
    "kanban-contract",
    "kanban-store-turso",
    "kanban-client",
    "kanban-cli",
    "kanban-mcp",
    "kanban-server",
)
DEFAULT_MEMBER_PATHS = tuple("crates/" + package for package in CORE_PACKAGES)
AUTO_TARGET_FLAGS = {
    "autobins": False,
    "autoexamples": False,
    "autotests": False,
    "autobenches": False,
    "autolib": False,
    "build": False,
}
TOOL_MANIFEST_LIB = {"name": "xtask", "path": "src/lib.rs"}
TOOL_MANIFEST_BINS = [{"name": "xtask", "path": "src/main.rs"}]
TOOL_MANIFEST_TESTS = [{"name": "tooling", "path": "tests/tooling.rs"}]
CONTRACT_MANIFEST_LIB = {"name": "kanban_contract", "path": "src/lib.rs"}
CONTRACT_MANIFEST_TESTS = [
    {"name": "foundation", "path": "tests/foundation.rs"},
    {"name": "g0_metadata", "path": "tests/g0_metadata.rs"},
]
APPROVED_ROOT_PATCH = {
    "crates-io": {
        "oxrdfxml": {"path": "vendor/oxrdfxml-0.2.3"},
        "sparesults": {"path": "vendor/sparesults-0.3.3"},
    }
}
TOOL_TARGETS = {
    ("xtask", ("lib",)): "xtask/src/lib.rs",
    ("xtask", ("bin",)): "xtask/src/main.rs",
    ("tooling", ("test",)): "xtask/tests/tooling.rs",
}
CONTRACT_TARGETS = {
    ("kanban_contract", ("lib",)): "crates/kanban-contract/src/lib.rs",
    ("foundation", ("test",)): "crates/kanban-contract/tests/foundation.rs",
    ("g0_metadata", ("test",)): "crates/kanban-contract/tests/g0_metadata.rs",
}
TOOL_TARGET_DISCOVERY_FILES = {
    "src/main.rs",
    "tests/tooling.rs",
}
CONTRACT_TARGET_DISCOVERY_FILES = {
    "tests/foundation.rs",
    "tests/g0_metadata.rs",
}
EDGE_SIGNATURE_FIELDS = (
    "name",
    "source",
    "path",
    "req",
    "kind",
    "rename",
    "optional",
    "uses_default_features",
    "features",
    "target",
    "registry",
)


class DependencyPolicyError(RuntimeError):
    """依赖拓扑偏离当前已批准 Phase 1 边界。"""


def _lexical_absolute(path: str | Path) -> Path:
    """归一化绝对路径，但不跟随任何 symlink。"""

    return Path(os.path.abspath(path))


def _assert_regular_repo_file(
    repo_root: Path, candidate: Path, package_name: str
) -> None:
    lexical_root = _lexical_absolute(repo_root)
    lexical_candidate = _lexical_absolute(candidate)
    try:
        relative = lexical_candidate.relative_to(lexical_root)
    except ValueError as error:
        raise DependencyPolicyError(
            f"{package_name} canonical target 越出仓库: {lexical_candidate}"
        ) from error

    current = lexical_root
    if current.is_symlink():
        raise DependencyPolicyError(f"仓库 root 禁止 symlink: {current}")
    for component in relative.parts:
        current /= component
        if current.is_symlink():
            raise DependencyPolicyError(
                f"{package_name} canonical target 路径组件禁止 symlink: {current}"
            )
    if not lexical_candidate.is_file():
        raise DependencyPolicyError(
            f"{package_name} canonical target 必须是普通文件: {lexical_candidate}"
        )


def _phase_two_message(detail: str) -> str:
    return (
        f"{detail}；Phase 2 拓扑变更必须先形成新决策并显式更新 gate，"
        "不能通过 manifest 或 metadata 漂移暗中扩边"
    )


def _audit_root_patch(workspace_manifest: dict[str, Any], repo_root: Path) -> None:
    """Allow only the explicitly approved temporary Oxigraph security patch."""
    patch = workspace_manifest.get("patch")
    if patch is None:
        return
    if patch != APPROVED_ROOT_PATCH:
        raise DependencyPolicyError(_phase_two_message(
            "root Cargo.toml [patch] 只能精确使用已批准的 Oxigraph security vendor override"
        ))
    versions = {"oxrdfxml": "0.2.3", "sparesults": "0.3.3"}
    for package, declaration in patch["crates-io"].items():
        target = repo_root / declaration["path"]
        _assert_regular_repo_file(repo_root, target / "Cargo.toml", package)
        with (target / "Cargo.toml").open("rb") as handle:
            manifest = tomllib.load(handle)
        metadata = manifest.get("package", {})
        if metadata.get("name") != package or metadata.get("version") != versions[package]:
            raise DependencyPolicyError(_phase_two_message(
                f"approved patch package identity/version 错误: {package}"
            ))


def _package_records_by_id(
    metadata: dict[str, Any],
) -> dict[str, dict[str, Any]]:
    packages = metadata.get("packages")
    if not isinstance(packages, list):
        raise DependencyPolicyError("cargo metadata 缺少 packages")

    package_by_id: dict[str, dict[str, Any]] = {}
    for record in packages:
        if not isinstance(record, dict):
            raise DependencyPolicyError(f"cargo metadata package record 格式无效: {record}")
        package_id = record.get("id")
        if not isinstance(package_id, str) or not package_id:
            raise DependencyPolicyError(f"cargo metadata package record 缺少 id: {record}")
        if package_id in package_by_id:
            raise DependencyPolicyError(f"cargo metadata package id 重复: {package_id}")
        package_by_id[package_id] = record
    return package_by_id


def _workspace_packages(metadata: dict[str, Any]) -> list[dict[str, Any]]:
    member_ids = metadata.get("workspace_members")
    if not isinstance(member_ids, list) or not all(
        isinstance(member_id, str) and member_id for member_id in member_ids
    ):
        raise DependencyPolicyError("cargo metadata 缺少有效 workspace_members")
    if len(member_ids) != len(set(member_ids)):
        raise DependencyPolicyError("cargo metadata workspace_members 存在重复 package id")

    package_by_id = _package_records_by_id(metadata)
    missing_ids = sorted(set(member_ids) - package_by_id.keys())
    if missing_ids:
        raise DependencyPolicyError(
            f"cargo metadata 缺少 workspace package records: {missing_ids}"
        )
    return [package_by_id[member_id] for member_id in member_ids]


def _workspace_package(
    workspace: list[dict[str, Any]], package_name: str
) -> dict[str, Any]:
    matches = [record for record in workspace if record.get("name") == package_name]
    if len(matches) != 1:
        raise DependencyPolicyError(
            f"workspace package identity 必须唯一: {package_name} ({len(matches)})"
        )
    return matches[0]


def _dependencies(package: dict[str, Any]) -> list[dict[str, Any]]:
    dependencies = package.get("dependencies")
    if not isinstance(dependencies, list) or not all(
        isinstance(dependency, dict) for dependency in dependencies
    ):
        package_name = package.get("name")
        raise DependencyPolicyError(f"package dependencies 格式无效: {package_name}")
    return dependencies


def _dependency_name(dependency: dict[str, Any]) -> str:
    name = dependency.get("name")
    if not isinstance(name, str) or not name:
        raise DependencyPolicyError(f"dependency 缺少 package name: {dependency}")
    return name


def _audit_package_targets(
    package: dict[str, Any],
    package_name: str,
    expected: dict[tuple[str, tuple[str, ...]], str],
    repo_root: Path,
) -> None:
    targets = package.get("targets")
    if not isinstance(targets, list) or not all(
        isinstance(target, dict) for target in targets
    ):
        raise DependencyPolicyError(f"{package_name} cargo metadata targets 格式无效")

    actual: dict[tuple[str, tuple[str, ...]], str] = {}
    for target in targets:
        name = target.get("name")
        kinds = target.get("kind")
        src_path = target.get("src_path")
        if (
            not isinstance(name, str)
            or not isinstance(kinds, list)
            or not all(isinstance(kind, str) for kind in kinds)
            or not isinstance(src_path, str)
        ):
            raise DependencyPolicyError(
                f"{package_name} cargo metadata target 格式无效: {target}"
            )
        key = (name, tuple(kinds))
        if key in actual:
            raise DependencyPolicyError(
                f"{package_name} cargo metadata target 重复: {key}"
            )
        actual[key] = str(_lexical_absolute(src_path))

    expected_resolved = {
        key: str(_lexical_absolute(repo_root / relative_path))
        for key, relative_path in expected.items()
    }
    for expected_path in expected_resolved.values():
        _assert_regular_repo_file(repo_root, Path(expected_path), package_name)
    if actual != expected_resolved:
        raise DependencyPolicyError(
            _phase_two_message(
                f"{package_name} target surface 必须精确锁定 name/kind/src_path: "
                f"expected={expected_resolved}, actual={actual}"
            )
        )


def _audit_resolved_feature_set(
    node: dict[str, Any], package_name: str, expected: tuple[str, ...]
) -> None:
    features = node.get("features")
    if not isinstance(features, list) or not all(
        isinstance(feature, str) for feature in features
    ):
        raise DependencyPolicyError(f"{package_name} resolve features 格式无效")
    if len(features) != len(set(features)) or sorted(features) != sorted(expected):
        raise DependencyPolicyError(
            _phase_two_message(
                f"{package_name} tool-root effective feature union 漂移: "
                f"expected={list(expected)}, actual={features}"
            )
        )


def _expected_tool_edge_signatures(
    repo_root: Path = ROOT,
) -> dict[str, dict[str, Any]]:
    common = {
        "kind": None,
        "rename": None,
        "optional": False,
        "target": None,
        "registry": None,
    }
    return {
        "jsonschema": {
            **common,
            "name": "jsonschema",
            "source": CRATES_IO_SOURCE,
            "path": None,
            "req": "^0.47.0",
            "uses_default_features": False,
            "features": [],
        },
        CONTRACT_PACKAGE: {
            **common,
            "name": CONTRACT_PACKAGE,
            "source": None,
            "path": str(_lexical_absolute(repo_root / "crates/kanban-contract")),
            "req": "*",
            "uses_default_features": False,
            "features": ["schema"],
        },
        "serde": {
            **common,
            "name": "serde",
            "source": CRATES_IO_SOURCE,
            "path": None,
            "req": "^1.0",
            "uses_default_features": True,
            "features": ["derive"],
        },
        "serde_json": {
            **common,
            "name": "serde_json",
            "source": CRATES_IO_SOURCE,
            "path": None,
            "req": "^1.0",
            "uses_default_features": True,
            "features": [],
        },
        "sha2": {
            **common,
            "name": "sha2",
            "source": CRATES_IO_SOURCE,
            "path": None,
            "req": "^0.10",
            "uses_default_features": True,
            "features": [],
        },
    }


def _expected_contract_edge_signatures() -> dict[str, dict[str, Any]]:
    common = {
        "kind": None,
        "rename": None,
        "target": None,
        "registry": None,
        "source": CRATES_IO_SOURCE,
        "path": None,
    }
    return {
        "schemars": {
            **common,
            "name": "schemars",
            "req": "^1.2.1",
            "optional": True,
            "uses_default_features": False,
            "features": ["std", "derive"],
        },
        "serde": {
            **common,
            "name": "serde",
            "req": "^1.0",
            "optional": False,
            "uses_default_features": True,
            "features": ["derive"],
        },
        "serde_json": {
            **common,
            "name": "serde_json",
            "req": "^1.0",
            "optional": False,
            "uses_default_features": True,
            "features": [],
        },
    }


def _edge_signature(dependency: dict[str, Any]) -> dict[str, Any]:
    return {field: dependency.get(field) for field in EDGE_SIGNATURE_FIELDS}


def _audit_non_tool_members(workspace: list[dict[str, Any]]) -> None:
    for package in workspace:
        package_name = package.get("name")
        if not isinstance(package_name, str) or not package_name:
            raise DependencyPolicyError(f"workspace package 缺少 name: {package}")
        if package_name == TOOL_PACKAGE:
            continue
        for dependency in _dependencies(package):
            if _dependency_name(dependency) != TOOL_PACKAGE:
                continue
            kind = dependency.get("kind") or "normal"
            alias = dependency.get("rename") or TOOL_PACKAGE
            target = dependency.get("target") or "all"
            optional = dependency.get("optional")
            raise DependencyPolicyError(
                _phase_two_message(
                    f"{package_name} 禁止通过 {kind} dependency {alias} 引用 "
                    f"{TOOL_PACKAGE} (target={target}, optional={optional})"
                )
            )


def _audit_tool_metadata(tool: dict[str, Any], repo_root: Path = ROOT) -> None:
    dependencies = _dependencies(tool)
    names = [_dependency_name(dependency) for dependency in dependencies]
    counts = Counter(names)
    missing = [name for name in TOOL_DIRECT_DEPENDENCY_NAMES if name not in counts]
    unexpected = [name for name in names if name not in TOOL_DIRECT_DEPENDENCY_NAMES]
    duplicates = sorted(name for name, count in counts.items() if count != 1)
    topology_changed = (
        len(dependencies) != len(TOOL_DIRECT_DEPENDENCY_NAMES)
        or missing
        or unexpected
        or duplicates
    )
    if topology_changed:
        raise DependencyPolicyError(
            _phase_two_message(
                "xtask direct normal dependencies 必须是精确五条唯一边: "
                f"missing={missing}, unexpected={unexpected}, duplicates={duplicates}, "
                f"actual_count={len(dependencies)}"
            )
        )

    expected_signatures = _expected_tool_edge_signatures(repo_root)
    for dependency in dependencies:
        name = _dependency_name(dependency)
        actual = _edge_signature(dependency)
        expected = expected_signatures[name]
        if actual != expected:
            raise DependencyPolicyError(
                _phase_two_message(
                    f"xtask dependency {name} signature 偏离 Phase 1: "
                    f"expected={expected}, actual={actual}"
                )
            )
    _audit_package_targets(tool, TOOL_PACKAGE, TOOL_TARGETS, repo_root)


def _audit_contract_metadata(
    contract: dict[str, Any], repo_root: Path = ROOT
) -> None:
    dependencies = _dependencies(contract)
    names = [_dependency_name(dependency) for dependency in dependencies]
    counts = Counter(names)
    missing = [name for name in CONTRACT_DIRECT_DEPENDENCY_NAMES if name not in counts]
    unexpected = [name for name in names if name not in CONTRACT_DIRECT_DEPENDENCY_NAMES]
    duplicates = sorted(name for name, count in counts.items() if count != 1)
    if (
        len(dependencies) != len(CONTRACT_DIRECT_DEPENDENCY_NAMES)
        or missing
        or unexpected
        or duplicates
    ):
        raise DependencyPolicyError(
            _phase_two_message(
                "kanban-contract schema dependency 声明必须精确为三条唯一边: "
                f"missing={missing}, unexpected={unexpected}, duplicates={duplicates}"
            )
        )

    expected_signatures = _expected_contract_edge_signatures()
    for dependency in dependencies:
        name = _dependency_name(dependency)
        actual = _edge_signature(dependency)
        expected = expected_signatures[name]
        if actual != expected:
            raise DependencyPolicyError(
                _phase_two_message(
                    f"kanban-contract dependency {name} signature 偏离 Phase 1: "
                    f"expected={expected}, actual={actual}"
                )
            )
    _audit_package_targets(contract, CONTRACT_PACKAGE, CONTRACT_TARGETS, repo_root)


def _resolve_nodes_by_id(
    metadata: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    resolve = metadata.get("resolve")
    if not isinstance(resolve, dict):
        raise DependencyPolicyError("cargo metadata 缺少完整 resolve graph")
    root = resolve.get("root")
    if not isinstance(root, str) or not root:
        raise DependencyPolicyError("cargo metadata resolve.root 缺少 package id")
    nodes = resolve.get("nodes")
    if not isinstance(nodes, list):
        raise DependencyPolicyError("cargo metadata resolve.nodes 格式无效")

    node_by_id: dict[str, dict[str, Any]] = {}
    for node in nodes:
        if not isinstance(node, dict):
            raise DependencyPolicyError(f"cargo metadata resolve node 格式无效: {node}")
        package_id = node.get("id")
        if not isinstance(package_id, str) or not package_id:
            raise DependencyPolicyError(f"cargo metadata resolve node 缺少 id: {node}")
        if package_id in node_by_id:
            raise DependencyPolicyError(f"cargo metadata resolve node id 重复: {package_id}")
        node_by_id[package_id] = node
    return resolve, node_by_id


def _resolved_edges(node: dict[str, Any], owner: str) -> list[dict[str, Any]]:
    dependencies = node.get("dependencies")
    deps = node.get("deps")
    features = node.get("features")
    if not isinstance(dependencies, list) or not all(
        isinstance(package_id, str) and package_id for package_id in dependencies
    ):
        raise DependencyPolicyError(f"{owner} resolve dependencies 格式无效")
    if not isinstance(deps, list) or not all(isinstance(edge, dict) for edge in deps):
        raise DependencyPolicyError(f"{owner} resolve deps 格式无效")
    if not isinstance(features, list) or not all(
        isinstance(feature, str) for feature in features
    ):
        raise DependencyPolicyError(f"{owner} resolve features 格式无效")

    package_ids: list[str] = []
    for edge in deps:
        alias = edge.get("name")
        package_id = edge.get("pkg")
        dep_kinds = edge.get("dep_kinds")
        if not isinstance(alias, str) or not alias:
            raise DependencyPolicyError(f"{owner} resolve edge 缺少 name: {edge}")
        if not isinstance(package_id, str) or not package_id:
            raise DependencyPolicyError(f"{owner} resolve edge 缺少 pkg: {edge}")
        if not isinstance(dep_kinds, list) or not all(
            isinstance(kind, dict)
            and set(kind) == {"kind", "target"}
            and (kind.get("kind") is None or isinstance(kind.get("kind"), str))
            and (kind.get("target") is None or isinstance(kind.get("target"), str))
            for kind in dep_kinds
        ):
            raise DependencyPolicyError(f"{owner} resolve dep_kinds 格式无效: {edge}")
        package_ids.append(package_id)
    if Counter(dependencies) != Counter(package_ids):
        raise DependencyPolicyError(
            f"{owner} resolve dependencies 与 deps[].pkg 映射不一致"
        )
    return deps


def _canonical_workspace_record(
    record: dict[str, Any], package_name: str, manifest_path: Path
) -> str:
    package_id = record.get("id")
    if not isinstance(package_id, str) or not package_id:
        raise DependencyPolicyError(f"{package_name} package id 无效")
    if record.get("source") is not None:
        raise DependencyPolicyError(
            _phase_two_message(f"{package_name} resolve source 必须是当前 workspace path")
        )
    actual_name = record.get("name")
    if actual_name != package_name:
        raise DependencyPolicyError(
            f"{package_name} workspace package record name 错误: {actual_name}"
        )
    actual_manifest = record.get("manifest_path")
    if (
        not isinstance(actual_manifest, str)
        or _lexical_absolute(actual_manifest)
        != _lexical_absolute(manifest_path)
    ):
        raise DependencyPolicyError(
            _phase_two_message(
                f"{package_name} resolve manifest_path 非 canonical: {actual_manifest}"
            )
        )
    return package_id


def _audit_resolved_direct_edges(
    node: dict[str, Any],
    owner: str,
    dependency_names: tuple[str, ...],
    package_by_id: dict[str, dict[str, Any]],
    expected_local_ids: dict[str, str],
) -> dict[str, str]:
    edges = _resolved_edges(node, owner)
    expected_aliases = {name.replace("-", "_"): name for name in dependency_names}
    aliases = [edge.get("name") for edge in edges]
    counts = Counter(aliases)
    if len(edges) != len(dependency_names) or counts != Counter(expected_aliases.keys()):
        raise DependencyPolicyError(
            _phase_two_message(
                f"{owner} resolved direct edges 必须一一对应 canonical declarations: "
                f"expected={sorted(expected_aliases)}, "
                f"actual={sorted(str(alias) for alias in aliases)}"
            )
        )

    resolved_ids: dict[str, str] = {}
    for edge in edges:
        alias = edge["name"]
        dependency_name = expected_aliases[alias]
        if edge.get("dep_kinds") != [{"kind": None, "target": None}]:
            raise DependencyPolicyError(
                _phase_two_message(
                    f"{owner} resolved edge {dependency_name} 必须是唯一 unconditional normal edge"
                )
            )
        package_id = edge["pkg"]
        record = package_by_id.get(package_id)
        if record is None:
            raise DependencyPolicyError(
                f"{owner} resolved edge {dependency_name} 缺少 package record: {package_id}"
            )
        if record.get("name") != dependency_name:
            raise DependencyPolicyError(
                _phase_two_message(
                    f"{owner} resolved edge {dependency_name} 指向错误 package: "
                    f"{record.get('name')}"
                )
            )
        expected_local_id = expected_local_ids.get(dependency_name)
        if expected_local_id is not None:
            if package_id != expected_local_id or record.get("source") is not None:
                raise DependencyPolicyError(
                    _phase_two_message(
                        f"{owner} resolved edge {dependency_name} 未指向 canonical workspace package"
                    )
                )
        elif record.get("source") != CRATES_IO_SOURCE:
            raise DependencyPolicyError(
                _phase_two_message(
                    f"{owner} resolved edge {dependency_name} 必须来自 crates.io: "
                    f"{record.get('source')}"
                )
            )
        resolved_ids[dependency_name] = package_id
    return resolved_ids


def _audit_tool_resolve_closure(
    tool_id: str,
    contract_id: str,
    package_by_id: dict[str, dict[str, Any]],
    node_by_id: dict[str, dict[str, Any]],
) -> set[str]:
    local_ids = {tool_id, contract_id}
    pending = [tool_id]
    visited: set[str] = set()
    while pending:
        package_id = pending.pop()
        if package_id in visited:
            continue
        visited.add(package_id)
        record = package_by_id.get(package_id)
        if record is None:
            raise DependencyPolicyError(
                f"schema tool resolve closure 缺少 package record: {package_id}"
            )
        node = node_by_id.get(package_id)
        if node is None:
            raise DependencyPolicyError(
                f"schema tool resolve closure 缺少 resolve node: {package_id}"
            )
        source = record.get("source")
        if package_id in local_ids:
            if source is not None:
                raise DependencyPolicyError(
                    f"schema tool resolve closure local package source 非 path: {package_id}"
                )
        elif source != CRATES_IO_SOURCE:
            raise DependencyPolicyError(
                _phase_two_message(
                    "schema tool resolve closure 只允许 canonical workspace tool/contract "
                    f"和 crates.io package: id={package_id}, source={source}"
                )
            )
        pending.extend(edge["pkg"] for edge in _resolved_edges(node, str(record.get("name"))))
    return visited


def audit_metadata(metadata: dict[str, Any], repo_root: Path = ROOT) -> set[str]:
    """锁定 schema leaf tool 的声明、resolved identity 与 crates.io 闭包。"""

    workspace = _workspace_packages(metadata)
    package_by_id = _package_records_by_id(metadata)
    tool = _workspace_package(workspace, TOOL_PACKAGE)
    contract = _workspace_package(workspace, CONTRACT_PACKAGE)
    _audit_non_tool_members(workspace)
    _audit_tool_metadata(tool, repo_root)
    _audit_contract_metadata(contract, repo_root)

    resolve, node_by_id = _resolve_nodes_by_id(metadata)
    tool_id = _canonical_workspace_record(
        tool, TOOL_PACKAGE, repo_root / "xtask/Cargo.toml"
    )
    contract_id = _canonical_workspace_record(
        contract, CONTRACT_PACKAGE, repo_root / "crates/kanban-contract/Cargo.toml"
    )
    if resolve.get("root") != tool_id:
        raise DependencyPolicyError(
            _phase_two_message(
                f"cargo metadata resolve.root 必须是 canonical {TOOL_PACKAGE}: "
                f"expected={tool_id}, actual={resolve.get('root')}"
            )
        )
    tool_node = node_by_id.get(tool_id)
    if tool_node is None:
        raise DependencyPolicyError(f"cargo metadata 缺少 {TOOL_PACKAGE} resolve node")
    contract_node = node_by_id.get(contract_id)
    if contract_node is None:
        raise DependencyPolicyError(f"cargo metadata 缺少 {CONTRACT_PACKAGE} resolve node")

    tool_edges = _audit_resolved_direct_edges(
        tool_node,
        TOOL_PACKAGE,
        TOOL_DIRECT_DEPENDENCY_NAMES,
        package_by_id,
        {CONTRACT_PACKAGE: contract_id},
    )
    jsonschema_id = tool_edges["jsonschema"]
    jsonschema = package_by_id[jsonschema_id]
    if jsonschema.get("version") != "0.47.0":
        raise DependencyPolicyError(
            _phase_two_message(
                "xtask 必须 resolve jsonschema 0.47.0: "
                f"{jsonschema.get('version')}"
            )
        )
    jsonschema_node = node_by_id.get(jsonschema_id)
    if jsonschema_node is None:
        raise DependencyPolicyError("cargo metadata 缺少 jsonschema 0.47.0 resolve node")
    _audit_resolved_feature_set(jsonschema_node, "jsonschema 0.47.0", ())

    contract_edges = _audit_resolved_direct_edges(
        contract_node,
        CONTRACT_PACKAGE,
        CONTRACT_DIRECT_DEPENDENCY_NAMES,
        package_by_id,
        {},
    )
    features = contract_node.get("features")
    if not isinstance(features, list) or "schema" not in features:
        raise DependencyPolicyError(
            _phase_two_message("kanban-contract resolve node 必须启用 schema feature")
        )
    schemars = package_by_id[contract_edges["schemars"]]
    if schemars.get("version") != "1.2.1":
        raise DependencyPolicyError(
            _phase_two_message(
                f"kanban-contract/schema 必须 resolve schemars 1.2.1: {schemars.get('version')}"
            )
        )
    schemars_node = node_by_id.get(contract_edges["schemars"])
    if schemars_node is None:
        raise DependencyPolicyError("cargo metadata 缺少 schemars 1.2.1 resolve node")
    _audit_resolved_feature_set(
        schemars_node,
        "schemars 1.2.1",
        ("chrono04", "default", "derive", "schemars_derive", "std"),
    )

    return _audit_tool_resolve_closure(
        tool_id, contract_id, package_by_id, node_by_id
    )


def _package_identity(
    record: dict[str, Any], owner: str
) -> tuple[str, str, str | None]:
    name = record.get("name")
    version = record.get("version")
    source = record.get("source")
    if not isinstance(name, str) or not name:
        raise DependencyPolicyError(f"{owner} package name 无效: {name}")
    if not isinstance(version, str) or not version:
        raise DependencyPolicyError(f"{owner} package version 无效: {version}")
    if source is not None and (not isinstance(source, str) or not source):
        raise DependencyPolicyError(f"{owner} package source 无效: {source}")
    return name, version, source


def _is_sha256(checksum: object) -> bool:
    return (
        isinstance(checksum, str)
        and re.fullmatch(r"[0-9a-f]{64}", checksum) is not None
    )


def _lock_packages_by_identity(
    lockfile: dict[str, Any],
) -> dict[tuple[str, str, str | None], dict[str, Any]]:
    if lockfile.get("version") != LOCKFILE_VERSION:
        raise DependencyPolicyError(
            f"Cargo.lock version 必须是 {LOCKFILE_VERSION}: {lockfile.get('version')}"
        )
    packages = lockfile.get("package")
    if not isinstance(packages, list):
        raise DependencyPolicyError("Cargo.lock 缺少 [[package]] records")

    packages_by_identity: dict[
        tuple[str, str, str | None], dict[str, Any]
    ] = {}
    for record in packages:
        if not isinstance(record, dict):
            raise DependencyPolicyError(
                f"Cargo.lock package record 格式无效: {record}"
            )
        identity = _package_identity(record, "Cargo.lock")
        if identity in packages_by_identity:
            raise DependencyPolicyError(
                f"Cargo.lock package identity 重复: {identity}"
            )
        packages_by_identity[identity] = record
    return packages_by_identity


def _approved_registry_packages(
    approved: dict[str, Any],
) -> list[dict[str, str]]:
    expected_top_fields = {
        "format_version",
        "lockfile_version",
        "root_package",
        "packages",
    }
    if set(approved) != expected_top_fields:
        raise DependencyPolicyError(
            "schema tool registry approval 顶层字段必须精确为 "
            f"{sorted(expected_top_fields)}: actual={sorted(approved)}"
        )
    if approved.get("format_version") != REGISTRY_CLOSURE_FORMAT_VERSION:
        raise DependencyPolicyError(
            "schema tool registry approval format_version 必须是 "
            f"{REGISTRY_CLOSURE_FORMAT_VERSION}: {approved.get('format_version')}"
        )
    if approved.get("lockfile_version") != LOCKFILE_VERSION:
        raise DependencyPolicyError(
            "schema tool registry approval lockfile_version 必须是 "
            f"{LOCKFILE_VERSION}: {approved.get('lockfile_version')}"
        )
    if approved.get("root_package") != TOOL_PACKAGE:
        raise DependencyPolicyError(
            "schema tool registry approval root_package 必须是 "
            f"{TOOL_PACKAGE}: {approved.get('root_package')}"
        )

    packages = approved.get("packages")
    if not isinstance(packages, list):
        raise DependencyPolicyError(
            "schema tool registry approval packages 必须是 array"
        )
    expected_record_fields = {"name", "version", "source", "checksum"}
    validated: list[dict[str, str]] = []
    identities: list[tuple[str, str, str]] = []
    for record in packages:
        if not isinstance(record, dict) or set(record) != expected_record_fields:
            actual_fields = sorted(record) if isinstance(record, dict) else record
            raise DependencyPolicyError(
                "schema tool registry approval package fields 必须精确为 "
                f"{sorted(expected_record_fields)}: actual={actual_fields}"
            )
        name, version, source = _package_identity(
            record, "schema tool registry approval"
        )
        if source is None:
            raise DependencyPolicyError(
                "schema tool registry approval 不得包含 workspace path package: "
                f"{name} {version}"
            )
        checksum = record.get("checksum")
        if not _is_sha256(checksum):
            raise DependencyPolicyError(
                "schema tool registry approval checksum 必须是 64 位小写十六进制: "
                f"{name} {version} {checksum}"
            )
        identities.append((name, version, source))
        validated.append(
            {
                "name": name,
                "version": version,
                "source": source,
                "checksum": checksum,
            }
        )
    if len(set(identities)) != len(identities):
        raise DependencyPolicyError(
            "schema tool registry approval package identity 重复"
        )
    if identities != sorted(identities):
        raise DependencyPolicyError(
            "schema tool registry approval packages 必须按 "
            "(name, version, source) canonical 排序"
        )
    return validated


def registry_closure_records(
    metadata: dict[str, Any],
    closure_ids: set[str],
    lockfile: dict[str, Any],
) -> list[dict[str, str]]:
    if not isinstance(closure_ids, set) or not all(
        isinstance(package_id, str) and package_id for package_id in closure_ids
    ):
        raise DependencyPolicyError("schema tool closure package IDs 无效")
    package_by_id = _package_records_by_id(metadata)
    lock_by_identity = _lock_packages_by_identity(lockfile)
    local_packages: set[str] = set()
    registry_records: list[dict[str, str]] = []
    for package_id in closure_ids:
        record = package_by_id.get(package_id)
        if record is None:
            raise DependencyPolicyError(
                f"schema tool registry closure 缺少 metadata package: {package_id}"
            )
        name, version, source = _package_identity(
            record, "schema tool metadata closure"
        )
        if source is None:
            local_packages.add(name)
            continue
        if source != CRATES_IO_SOURCE:
            raise DependencyPolicyError(
                _phase_two_message(
                    "schema tool registry closure source 非批准 logical SourceId: "
                    f"{name} {version} {source}"
                )
            )
        identity = (name, version, source)
        lock_record = lock_by_identity.get(identity)
        if lock_record is None:
            raise DependencyPolicyError(
                "Cargo.lock 缺少 schema tool reachable registry package: "
                f"{identity}"
            )
        checksum = lock_record.get("checksum")
        if not _is_sha256(checksum):
            raise DependencyPolicyError(
                "Cargo.lock reachable registry checksum 必须是 64 位小写十六进制: "
                f"{name} {version} {checksum}"
            )
        registry_records.append(
            {
                "name": name,
                "version": version,
                "source": source,
                "checksum": checksum,
            }
        )
    expected_local_packages = {TOOL_PACKAGE, CONTRACT_PACKAGE}
    if local_packages != expected_local_packages:
        raise DependencyPolicyError(
            "schema tool closure path package 必须精确为 tool/contract: "
            f"expected={sorted(expected_local_packages)}, "
            f"actual={sorted(local_packages)}"
        )
    registry_records.sort(
        key=lambda record: (
            record["name"],
            record["version"],
            record["source"],
        )
    )
    return registry_records


def audit_registry_closure_snapshot(
    metadata: dict[str, Any],
    closure_ids: set[str],
    lockfile: dict[str, Any],
    approved: dict[str, Any],
) -> None:
    actual = registry_closure_records(metadata, closure_ids, lockfile)
    expected = _approved_registry_packages(approved)
    if actual == expected:
        return

    actual_by_identity = {
        (record["name"], record["version"], record["source"]): record
        for record in actual
    }
    expected_by_identity = {
        (record["name"], record["version"], record["source"]): record
        for record in expected
    }
    missing = sorted(set(expected_by_identity) - set(actual_by_identity))
    unexpected = sorted(set(actual_by_identity) - set(expected_by_identity))
    checksum_drift = sorted(
        identity
        for identity in set(actual_by_identity) & set(expected_by_identity)
        if actual_by_identity[identity]["checksum"]
        != expected_by_identity[identity]["checksum"]
    )
    raise DependencyPolicyError(
        "schema tool reachable registry closure 偏离 approved snapshot: "
        f"missing={missing}, unexpected={unexpected}, "
        f"checksum_drift={checksum_drift}"
    )


def audit_manifest_data(
    workspace_manifest: dict[str, Any],
    tool_manifest: dict[str, Any],
    repo_root: Path = ROOT,
) -> None:
    """锁定产生 metadata signature 的 canonical manifest 声明。"""

    workspace = workspace_manifest.get("workspace")
    if not isinstance(workspace, dict):
        raise DependencyPolicyError("root Cargo.toml 缺少 [workspace]")
    members = workspace.get("members")
    defaults = workspace.get("default-members")
    if not isinstance(members, list) or members.count(TOOL_MEMBER_PATH) != 1:
        raise DependencyPolicyError(
            "workspace.members 必须精确包含一个 private xtask leaf"
        )
    if defaults != list(DEFAULT_MEMBER_PATHS):
        raise DependencyPolicyError(
            _phase_two_message(
                "workspace.default-members 必须精确为 canonical 产品集并排除 schema tool"
            )
        )
    _audit_root_patch(workspace_manifest, repo_root)
    if "replace" in workspace_manifest:
        raise DependencyPolicyError(_phase_two_message("root Cargo.toml 禁止声明 [replace] override"))
    workspace_dependencies = workspace.get("dependencies")
    if not isinstance(workspace_dependencies, dict):
        raise DependencyPolicyError("root Cargo.toml 缺少 [workspace.dependencies]")
    for name, expected in WORKSPACE_CANONICAL_DEPENDENCIES.items():
        actual = workspace_dependencies.get(name)
        if actual != expected:
            raise DependencyPolicyError(
                _phase_two_message(
                    f"workspace canonical dependency {name} 偏离 Phase 1: "
                    f"expected={expected}, actual={actual}"
                )
            )

    package = tool_manifest.get("package")
    if not isinstance(package, dict):
        raise DependencyPolicyError("xtask Cargo.toml 缺少 [package]")
    if package.get("name") != TOOL_PACKAGE or package.get("publish") is not False:
        raise DependencyPolicyError(
            _phase_two_message(
                "xtask package identity 必须固定且 publish = false"
            )
        )
    actual_flags = {name: package.get(name) for name in AUTO_TARGET_FLAGS}
    if actual_flags != AUTO_TARGET_FLAGS:
        raise DependencyPolicyError(
            _phase_two_message(
                "xtask 必须关闭全部 Cargo auto target discovery: "
                f"expected={AUTO_TARGET_FLAGS}, actual={actual_flags}"
            )
        )
    if tool_manifest.get("lib") != TOOL_MANIFEST_LIB:
        raise DependencyPolicyError(
            _phase_two_message("xtask 必须只声明 canonical lib target")
        )
    if tool_manifest.get("bin") != TOOL_MANIFEST_BINS:
        raise DependencyPolicyError(
            _phase_two_message("xtask 必须只拥有 canonical xtask binary")
        )
    if tool_manifest.get("test") != TOOL_MANIFEST_TESTS:
        raise DependencyPolicyError(
            _phase_two_message("xtask 必须只声明 canonical tooling test target")
        )
    if "features" in tool_manifest:
        raise DependencyPolicyError(
            _phase_two_message("xtask 禁止声明 feature surface")
        )
    for forbidden_section in (
        "dev-dependencies",
        "build-dependencies",
        "target",
        "example",
        "bench",
    ):
        if forbidden_section in tool_manifest:
            raise DependencyPolicyError(
                _phase_two_message(
                    f"xtask 禁止声明 [{forbidden_section}] 依赖入口"
                )
            )

    actual_dependencies = tool_manifest.get("dependencies")
    if actual_dependencies != TOOL_MANIFEST_DEPENDENCIES:
        raise DependencyPolicyError(
            _phase_two_message(
                "xtask [dependencies] 必须精确使用五条 workspace canonical 声明: "
                f"expected={TOOL_MANIFEST_DEPENDENCIES}, actual={actual_dependencies}"
            )
        )

    expected_contract_path = _lexical_absolute(
        repo_root / "crates/kanban-contract"
    )
    if not expected_contract_path.is_dir():
        raise DependencyPolicyError(
            f"canonical kanban-contract path 不存在: {expected_contract_path}"
        )


def audit_contract_manifest_data(
    workspace_manifest: dict[str, Any],
    contract_manifest: dict[str, Any],
    repo_root: Path = ROOT,
) -> None:
    """锁定 model contract 的唯一 schema feature 与 canonical schemars 声明。"""

    workspace = workspace_manifest.get("workspace")
    if not isinstance(workspace, dict):
        raise DependencyPolicyError("root Cargo.toml 缺少 [workspace]")
    _audit_root_patch(workspace_manifest, repo_root)
    if "replace" in workspace_manifest:
        raise DependencyPolicyError(_phase_two_message("root Cargo.toml 禁止声明 [replace] override"))
    workspace_dependencies = workspace.get("dependencies")
    if not isinstance(workspace_dependencies, dict):
        raise DependencyPolicyError("root Cargo.toml 缺少 [workspace.dependencies]")
    expected_schemars = WORKSPACE_CANONICAL_DEPENDENCIES["schemars"]
    actual_schemars = workspace_dependencies.get("schemars")
    if actual_schemars != expected_schemars:
        raise DependencyPolicyError(
            _phase_two_message(
                "workspace canonical schemars 必须固定 1.2.1/default=false/std+derive: "
                f"expected={expected_schemars}, actual={actual_schemars}"
            )
        )

    package = contract_manifest.get("package")
    if not isinstance(package, dict) or package.get("name") != CONTRACT_PACKAGE:
        raise DependencyPolicyError("kanban-contract Cargo.toml package identity 无效")
    actual_flags = {name: package.get(name) for name in AUTO_TARGET_FLAGS}
    if actual_flags != AUTO_TARGET_FLAGS:
        raise DependencyPolicyError(
            _phase_two_message(
                "kanban-contract 必须关闭全部 Cargo auto target discovery: "
                f"expected={AUTO_TARGET_FLAGS}, actual={actual_flags}"
            )
        )
    if contract_manifest.get("lib") != CONTRACT_MANIFEST_LIB:
        raise DependencyPolicyError(
            _phase_two_message("kanban-contract 必须只声明 canonical lib target")
        )
    if contract_manifest.get("test") != CONTRACT_MANIFEST_TESTS:
        raise DependencyPolicyError(
            _phase_two_message(
                "kanban-contract 必须只声明 foundation/g0_metadata test targets"
            )
        )
    if contract_manifest.get("features") != CONTRACT_MANIFEST_FEATURES:
        raise DependencyPolicyError(
            _phase_two_message(
                "kanban-contract features 必须精确为 default=[] 与 schema=[dep:schemars]"
            )
        )
    if contract_manifest.get("dependencies") != CONTRACT_MANIFEST_DEPENDENCIES:
        raise DependencyPolicyError(
            _phase_two_message(
                "kanban-contract dependencies 必须精确为 serde/serde_json 和 optional schemars"
            )
        )
    if "bin" in contract_manifest:
        raise DependencyPolicyError(
            _phase_two_message("kanban-contract 不得拥有 binary")
        )
    for forbidden_section in (
        "dev-dependencies",
        "build-dependencies",
        "target",
        "example",
        "bench",
    ):
        if forbidden_section in contract_manifest:
            raise DependencyPolicyError(
                _phase_two_message(
                    f"kanban-contract 禁止声明 [{forbidden_section}] 依赖入口"
                )
            )


def audit_target_files(repo_root: Path = ROOT) -> None:
    """拒绝 manifest 之外仍可被 Cargo 识别或误启用的 target 文件。"""

    def discovery_files(package_root: Path) -> set[str]:
        discovered: set[str] = set()
        for relative_dir in ("src/bin", "tests", "examples", "benches"):
            directory = package_root / relative_dir
            if directory.is_symlink():
                raise DependencyPolicyError(
                    f"target discovery directory 禁止 symlink: {directory}"
                )
            if not directory.exists():
                continue
            for candidate in directory.rglob("*"):
                if candidate.is_symlink():
                    raise DependencyPolicyError(
                        f"target discovery file 禁止 symlink: {candidate}"
                    )
                if candidate.is_file():
                    discovered.add(candidate.relative_to(package_root).as_posix())
        for relative_file in ("build.rs", "src/main.rs"):
            candidate = package_root / relative_file
            if candidate.is_symlink():
                raise DependencyPolicyError(
                    f"target discovery file 禁止 symlink: {candidate}"
                )
            if candidate.exists():
                discovered.add(relative_file)
        return discovered

    surfaces = (
        (
            TOOL_PACKAGE,
            repo_root / "xtask",
            TOOL_TARGET_DISCOVERY_FILES,
        ),
        (
            CONTRACT_PACKAGE,
            repo_root / "crates/kanban-contract",
            CONTRACT_TARGET_DISCOVERY_FILES,
        ),
    )
    for package_name, package_root, expected in surfaces:
        approved = {"src/lib.rs", *expected}
        for relative_path in approved:
            _assert_regular_repo_file(
                repo_root, package_root / relative_path, package_name
            )
        actual = discovery_files(package_root)
        if actual != expected:
            raise DependencyPolicyError(
                _phase_two_message(
                    f"{package_name} target discovery files 漂移: "
                    f"expected={sorted(expected)}, actual={sorted(actual)}"
                )
            )


def audit_manifests(repo_root: Path = ROOT) -> None:
    try:
        with (repo_root / "Cargo.toml").open("rb") as handle:
            workspace_manifest = tomllib.load(handle)
        with (repo_root / "crates/kanban-contract/Cargo.toml").open("rb") as handle:
            contract_manifest = tomllib.load(handle)
        with (repo_root / "xtask/Cargo.toml").open("rb") as handle:
            tool_manifest = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise DependencyPolicyError(f"读取 schema dependency policy inputs 失败: {error}") from error
    audit_manifest_data(workspace_manifest, tool_manifest, repo_root)
    audit_contract_manifest_data(workspace_manifest, contract_manifest, repo_root)
    audit_target_files(repo_root)


def load_lockfile(repo_root: Path = ROOT) -> dict[str, Any]:
    path = repo_root / "Cargo.lock"
    _assert_regular_repo_file(repo_root, path, "schema dependency policy")
    try:
        with path.open("rb") as handle:
            lockfile = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise DependencyPolicyError(f"读取 Cargo.lock 失败: {error}") from error
    if not isinstance(lockfile, dict):
        raise DependencyPolicyError("Cargo.lock 顶层必须是 table")
    return lockfile


def load_approved_registry_closure(
    repo_root: Path = ROOT,
) -> dict[str, Any]:
    path = repo_root / APPROVED_REGISTRY_CLOSURE_PATH
    _assert_regular_repo_file(
        repo_root, path, "schema tool registry approval"
    )
    try:
        with path.open(encoding="utf-8") as handle:
            approved = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise DependencyPolicyError(
            f"读取 {APPROVED_REGISTRY_CLOSURE_PATH} 失败: {error}"
        ) from error
    if not isinstance(approved, dict):
        raise DependencyPolicyError(
            "schema tool registry approval 顶层必须是 object"
        )
    return approved


def _inherited_lock_pass_fds() -> tuple[int, ...]:
    lock_fd_raw = os.environ.get("KANBAN_CARGO_BUILD_LOCK_FD", "")
    if not (
        os.environ.get("KANBAN_CARGO_BUILD_LOCK_HELD") == "1"
        and lock_fd_raw.isascii()
        and lock_fd_raw.isdecimal()
        and lock_fd_raw[0] in "3456789"
    ):
        return ()
    try:
        lock_fd = int(lock_fd_raw)
        os.fstat(lock_fd)
    except (OSError, OverflowError, ValueError):
        return ()
    return (lock_fd,)


def load_metadata(repo_root: Path = ROOT) -> dict[str, Any]:
    command = [
        str(repo_root / "scripts/cargo-build-lock.sh"),
        "--",
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--locked",
        "--manifest-path",
        "xtask/Cargo.toml",
    ]
    completed = subprocess.run(
        command,
        cwd=repo_root,
        check=False,
        close_fds=True,
        pass_fds=_inherited_lock_pass_fds(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise DependencyPolicyError(
            f"cargo metadata 失败 ({completed.returncode}): {completed.stderr.strip()}"
        )
    try:
        metadata = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise DependencyPolicyError(f"cargo metadata JSON 无效: {error}") from error
    if not isinstance(metadata, dict):
        raise DependencyPolicyError("cargo metadata 顶层必须是 object")
    return metadata


def main() -> int:
    try:
        audit_manifests()
        metadata = load_metadata()
        closure_ids = audit_metadata(metadata)
        audit_registry_closure_snapshot(
            metadata,
            closure_ids,
            load_lockfile(),
            load_approved_registry_closure(),
        )
    except DependencyPolicyError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print("ok: Phase 1 schema tooling 声明、resolved identity 与产品隔离已锁定")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
