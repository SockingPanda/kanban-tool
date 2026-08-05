#!/usr/bin/env python3
"""校验 schema runtime adoption 的依赖与精确测试 witness。"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN_SCHEMA_FEATURES = {"schema", "schema-tool"}
CONTRACT_PACKAGE = "kanban-protocol"
SCHEMA_TOOL_PACKAGE = "xtask"
CONTRACT_MANIFEST = Path("crates/kanban-protocol/Cargo.toml")


class WitnessGateError(RuntimeError):
    """adoption witness 无法被执行时返回。"""


def package_record(metadata: dict[str, Any], package: str) -> dict[str, Any]:
    workspace_members = {
        member
        for member in metadata.get("workspace_members", [])
        if isinstance(member, str)
    }
    if not workspace_members:
        raise WitnessGateError("cargo metadata 缺少 workspace_members")
    matches = [
        item
        for item in metadata.get("packages", [])
        if isinstance(item, dict)
        and item.get("name") == package
        and item.get("id") in workspace_members
    ]
    if len(matches) != 1:
        raise WitnessGateError(
            f"witness package 必须精确匹配一个 workspace package: {package}"
        )
    return matches[0]


def metadata_workspace_root(
    metadata: dict[str, Any], repo_root: Path | None
) -> Path:
    if repo_root is not None:
        return repo_root.resolve()
    workspace_root = metadata.get("workspace_root")
    if not isinstance(workspace_root, str) or not workspace_root:
        raise WitnessGateError("cargo metadata 缺少 workspace_root")
    return Path(workspace_root).resolve()


def workspace_contract_identity(
    metadata: dict[str, Any], repo_root: Path | None = None
) -> tuple[str, Path]:
    root = metadata_workspace_root(metadata, repo_root)
    record = package_record(metadata, CONTRACT_PACKAGE)
    package_id = record.get("id")
    manifest_path = record.get("manifest_path")
    expected_manifest = (root / CONTRACT_MANIFEST).resolve()
    actual_manifest = (
        Path(manifest_path).resolve()
        if isinstance(manifest_path, str) and manifest_path
        else None
    )
    if (
        not isinstance(package_id, str)
        or record.get("source") is not None
        or actual_manifest != expected_manifest
    ):
        raise WitnessGateError(
            "cargo metadata 中的 workspace kanban-protocol identity 不可信: "
            f"expected_manifest={expected_manifest}"
        )
    return package_id, expected_manifest.parent


def resolve_node(
    metadata: dict[str, Any], package_id: str
) -> dict[str, Any]:
    resolve = metadata.get("resolve")
    nodes = resolve.get("nodes", []) if isinstance(resolve, dict) else []
    matches = [
        node
        for node in nodes
        if isinstance(node, dict) and node.get("id") == package_id
    ]
    if len(matches) != 1:
        raise WitnessGateError(
            f"cargo metadata resolve graph 缺少唯一 package node: {package_id}"
        )
    return matches[0]


def require_runtime_dependency(
    metadata: dict[str, Any],
    package: str,
    repo_root: Path | None = None,
) -> tuple[str, Path]:
    """要求 adopter 通过 normal dependency 引用当前 workspace kanban-protocol。"""

    if package == SCHEMA_TOOL_PACKAGE:
        raise WitnessGateError(
            f"schema tooling owner 不能充当 runtime adopter: {SCHEMA_TOOL_PACKAGE}"
        )

    record = package_record(metadata, package)
    adopter_id = record.get("id")
    if not isinstance(adopter_id, str):
        raise WitnessGateError(f"witness package 缺少 package id: {package}")

    contract_id, contract_path = workspace_contract_identity(metadata, repo_root)
    dependencies = [
        dependency
        for dependency in record.get("dependencies", [])
        if isinstance(dependency, dict)
        and dependency.get("name") == CONTRACT_PACKAGE
    ]
    normal_dependencies = [
        dependency for dependency in dependencies if dependency.get("kind") is None
    ]
    unconditional_normal_dependencies = [
        dependency
        for dependency in normal_dependencies
        if dependency.get("target") is None
        and not dependency.get("optional", False)
    ]
    if not unconditional_normal_dependencies:
        raise WitnessGateError(
            f"runtime adopter {package} 必须有 kanban-protocol unconditional non-optional normal dependency，"
            "dev-only、optional 或 target-specific dependency 不能证明运行时采用"
        )

    workspace_dependencies = [
        dependency
        for dependency in unconditional_normal_dependencies
        if dependency.get("source") is None
        and isinstance(dependency.get("path"), str)
        and Path(dependency["path"]).resolve() == contract_path
    ]
    if not workspace_dependencies:
        raise WitnessGateError(
            f"runtime adopter {package} normal dependency 必须指向当前 "
            f"workspace kanban-protocol: {contract_path}"
        )

    forbidden = sorted(
        {
            feature
            for dependency in workspace_dependencies
            for feature in dependency.get("features", [])
            if feature in FORBIDDEN_SCHEMA_FEATURES
        }
    )
    if forbidden:
        raise WitnessGateError(
            f"runtime adopter {package} 禁止启用 kanban-protocol schema feature: "
            f"{', '.join(forbidden)}"
        )

    node = resolve_node(metadata, adopter_id)
    resolved_dependencies = [
        dependency
        for dependency in node.get("deps", [])
        if isinstance(dependency, dict)
        and dependency.get("pkg") == contract_id
        and any(
            isinstance(kind, dict)
            and kind.get("kind") is None
            and kind.get("target") is None
            for kind in dependency.get("dep_kinds", [])
        )
    ]
    if not resolved_dependencies:
        raise WitnessGateError(
            f"runtime adopter {package} 的 unconditional non-optional normal dependency package identity "
            f"未解析到 workspace kanban-protocol: {contract_id}"
        )
    return adopter_id, contract_path


def require_runtime_tree(
    tree_output: str, package: str, contract_path: Path | None = None
) -> None:
    forbidden = re.search(
        r'xtask v|kanban-protocol feature "schema(?:-tool)?"|schemars v1\.|jsonschema v',
        tree_output,
    )
    if forbidden is not None:
        raise WitnessGateError(
            f"runtime adopter {package} 默认 normal graph 泄漏 schema tooling: "
            f"{forbidden.group(0)}"
        )
    contract_lines = [
        line for line in tree_output.splitlines() if "kanban-protocol v" in line
    ]
    if not contract_lines:
        raise WitnessGateError(
            f"runtime adopter {package} 默认 normal graph 未出现 kanban-protocol"
        )
    if contract_path is not None and not any(
        f"({contract_path})" in line for line in contract_lines
    ):
        raise WitnessGateError(
            f"runtime adopter {package} normal graph 未出现 workspace kanban-protocol"
        )


def test_target_selector(
    metadata: dict[str, Any], package: str, test_target: str
) -> list[str]:
    record = package_record(metadata, package)
    targets = [target for target in record.get("targets", []) if isinstance(target, dict)]
    if test_target == "lib":
        if any("lib" in target.get("kind", []) for target in targets):
            return ["--lib"]
    elif any(
        target.get("name") == test_target and "test" in target.get("kind", [])
        for target in targets
    ):
        return ["--test", test_target]
    raise WitnessGateError(
        f"witness test target 不存在: package={package}, test_target={test_target}"
    )


def require_test_target(
    metadata: dict[str, Any], package: str, test_target: str
) -> None:
    """要求 `lib` 或具名 integration test target 存在。"""

    test_target_selector(metadata, package, test_target)


def require_exact_test(list_output: str, exact_test: str) -> None:
    """要求 libtest `--exact --list` 精确返回一个 test。"""

    expected = f"{exact_test}: test"
    matches = [line.strip() for line in list_output.splitlines() if line.strip() == expected]
    if len(matches) != 1:
        raise WitnessGateError(
            f"精确 witness locator 返回 {len(matches)} tests，预期 1: {exact_test}"
        )


def require_executed_test(run_output: str, exact_test: str) -> None:
    """要求精确 locator 真实执行且通过一个 test。"""

    executed = any(
        line.strip().startswith(f"test {exact_test} ... ok")
        for line in run_output.splitlines()
    )
    summary = re.search(
        r"test result: ok\. 1 passed; 0 failed; 0 ignored;", run_output
    )
    if not executed or summary is None:
        raise WitnessGateError(
            f"精确 witness locator 未真实执行 1 test: {exact_test}"
        )


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


def run_checked(command: list[str], repo_root: Path) -> str:
    completed = subprocess.run(
        command,
        cwd=repo_root,
        close_fds=True,
        pass_fds=_inherited_lock_pass_fds(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise WitnessGateError(
            f"命令失败（exit={completed.returncode}）: {' '.join(command)}\n{detail}"
        )
    return completed.stdout


def load_witness_plan(repo_root: Path) -> list[dict[str, Any]]:
    output = run_checked(
        [
            str(repo_root / "scripts/cargo-build-lock.sh"),
            "--",
            "cargo",
            "run",
            "--locked",
            "--quiet",
            "-p",
            "xtask",
            "--bin",
            "xtask",
            "--",
            "schema",
            "witnesses",
            "--root",
            str(repo_root),
        ],
        repo_root,
    )
    try:
        plan = json.loads(output)
    except json.JSONDecodeError as error:
        raise WitnessGateError(f"witness plan 不是合法 JSON: {error}") from error
    if not isinstance(plan, list):
        raise WitnessGateError("witness plan 顶层必须是 array")
    return plan


def load_cargo_metadata(repo_root: Path) -> dict[str, Any]:
    output = run_checked(
        [
            str(repo_root / "scripts/cargo-build-lock.sh"),
            "--",
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--locked",
        ],
        repo_root,
    )
    metadata = json.loads(output)
    if not isinstance(metadata, dict):
        raise WitnessGateError("cargo metadata 顶层必须是 object")
    return metadata


def witness_locator(contract: dict[str, Any], role: str) -> tuple[str, str, str]:
    adoption = contract.get("adoption")
    if not isinstance(adoption, dict):
        raise WitnessGateError(f"adopted contract 缺少 adoption object: {contract.get('id')}")
    witness = adoption.get(role)
    if not isinstance(witness, dict):
        raise WitnessGateError(
            f"adopted contract 缺少 {role} witness: {contract.get('id')}"
        )
    values = tuple(witness.get(field) for field in ("package", "test_target", "exact_test"))
    if not all(isinstance(value, str) and value.strip() for value in values):
        raise WitnessGateError(
            f"adopted contract {role} witness locator 不完整: {contract.get('id')}"
        )
    return values[0], values[1], values[2]


def validate_runtime_graph(
    repo_root: Path, metadata: dict[str, Any], package: str
) -> None:
    adopter_id, contract_path = require_runtime_dependency(
        metadata, package, repo_root
    )
    tree_output = run_checked(
        [
            str(repo_root / "scripts/cargo-build-lock.sh"),
            "--",
            "cargo",
            "tree",
            "-p",
            adopter_id,
            "--all-features",
            "--target",
            "all",
            "--edges",
            "normal,features",
            "--locked",
        ],
        repo_root,
    )
    require_runtime_tree(tree_output, package, contract_path)


def execute_witness(
    repo_root: Path,
    metadata: dict[str, Any],
    locator: tuple[str, str, str],
) -> None:
    package, test_target, exact_test = locator
    adopter_id, _ = require_runtime_dependency(
        metadata, package, repo_root
    )
    selector = test_target_selector(metadata, package, test_target)
    command = [
        str(repo_root / "scripts/cargo-build-lock.sh"),
        "--",
        "cargo",
        "test",
        "--locked",
        "-p",
        adopter_id,
        *selector,
        exact_test,
        "--",
        "--exact",
    ]
    list_output = run_checked([*command, "--list"], repo_root)
    require_exact_test(list_output, exact_test)
    run_output = run_checked(command, repo_root)
    require_executed_test(run_output, exact_test)


def execute_witness_group(
    repo_root: Path,
    metadata: dict[str, Any],
    package: str,
    test_target: str,
    exact_tests: list[str],
) -> None:
    adopter_id, _ = require_runtime_dependency(metadata, package, repo_root)
    selector = test_target_selector(metadata, package, test_target)
    command = [
        str(repo_root / "scripts/cargo-build-lock.sh"),
        "--",
        "cargo",
        "test",
        "--locked",
        "-p",
        adopter_id,
        *selector,
        "--",
    ]
    list_output = run_checked([*command, "--list"], repo_root)
    for exact_test in exact_tests:
        require_exact_test(list_output, exact_test)
    run_output = run_checked(command, repo_root)
    for exact_test in exact_tests:
        if not any(
            line.strip().startswith(f"test {exact_test} ... ok")
            for line in run_output.splitlines()
        ):
            raise WitnessGateError(f"witness locator 未真实执行并通过: {exact_test}")


def execute_unique_witnesses(
    repo_root: Path,
    metadata: dict[str, Any],
    witnesses: list[tuple[str, str, tuple[str, str, str]]],
) -> list[dict[str, Any]]:
    groups: dict[tuple[str, str], set[str]] = {}
    for package, test_target, exact_test in {
        locator for _, _, locator in witnesses
    }:
        groups.setdefault((package, test_target), set()).add(exact_test)
    for (package, test_target), exact_tests in sorted(groups.items()):
        execute_witness_group(
            repo_root, metadata, package, test_target, sorted(exact_tests)
        )
    return [
        {"contract_id": contract_id, "role": role, "locator": locator}
        for contract_id, role, locator in witnesses
    ]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="repository root")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = args.root.resolve()
    contracts = load_witness_plan(repo_root)
    if not contracts:
        print("ok: 当前 0 个 adopted contract，witness gate 空集通过")
        return 0

    metadata = load_cargo_metadata(repo_root)
    packages: set[str] = set()
    witnesses: list[tuple[str, str, tuple[str, str, str]]] = []
    for contract in contracts:
        if not isinstance(contract, dict) or contract.get("migration") != "adopted":
            raise WitnessGateError("witness plan 只能包含 adopted contract")
        for role in ("producer", "consumer"):
            locator = witness_locator(contract, role)
            packages.add(locator[0])
            witnesses.append((str(contract.get("id")), role, locator))

    for package in sorted(packages):
        validate_runtime_graph(repo_root, metadata, package)
    reports = execute_unique_witnesses(repo_root, metadata, witnesses)
    for report in reports:
        contract_id = report["contract_id"]
        role = report["role"]
        locator = report["locator"]
        print(
            f"ok: {contract_id} {role} witness package={locator[0]} "
            f"test_target={locator[1]} exact_test={locator[2]}"
        )
    print(
        f"ok: {len(contracts)} 个 adopted contract 的 {len(witnesses)} 个 mapping "
        f"由 {len({locator for _, _, locator in witnesses})} 个 unique locator 执行"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except WitnessGateError as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
